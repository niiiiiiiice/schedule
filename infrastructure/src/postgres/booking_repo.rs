use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use application::errors::AppError;
use application::scheduler::ports::BookingRepository;
use domain::scheduler::booking::{Booking, BookingStatus};

pub struct PgBookingRepository {
    pool: PgPool,
}

impl PgBookingRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct BookingRow {
    id: Uuid,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    status: String,
    metadata: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Convert UUID to i64 advisory lock key (first 8 bytes).
fn uuid_advisory_key(id: Uuid) -> i64 {
    let b = id.as_bytes();
    i64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

async fn load_resource_ids(
    pool: &PgPool,
    booking_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let rows: Vec<(Uuid,)> =
        sqlx::query_as("SELECT resource_id FROM booking_resources WHERE booking_id = $1 ORDER BY resource_id")
            .bind(booking_id)
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

fn row_to_booking(row: BookingRow, resource_ids: Vec<Uuid>) -> Booking {
    let status = if row.status == "confirmed" {
        BookingStatus::Confirmed
    } else {
        BookingStatus::Cancelled
    };
    Booking::from_raw(
        row.id,
        resource_ids,
        row.start_at,
        row.end_at,
        status,
        row.metadata,
        row.created_at,
        row.updated_at,
    )
}

#[async_trait]
impl BookingRepository for PgBookingRepository {
    async fn create_atomic(
        &self,
        booking: &Booking,
        resource_max_concurrent: &[(Uuid, i32)],
    ) -> Result<(), AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        // Sort by UUID bytes for deterministic lock order (deadlock prevention)
        let mut sorted = resource_max_concurrent.to_vec();
        sorted.sort_by_key(|(id, _)| *id);

        for (resource_id, max_concurrent) in &sorted {
            // Acquire advisory transaction lock
            let key = uuid_advisory_key(*resource_id);
            sqlx::query("SELECT pg_advisory_xact_lock($1)")
                .bind(key)
                .execute(&mut *tx)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

            // Check capacity (double-checked inside lock)
            let count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM booking_resources br
                JOIN bookings b ON b.id = br.booking_id
                WHERE br.resource_id = $1
                  AND b.status = 'confirmed'
                  AND br.start_at < $3
                  AND br.end_at > $2
                "#,
            )
            .bind(resource_id)
            .bind(booking.start_at)
            .bind(booking.end_at)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

            if count >= *max_concurrent as i64 {
                tx.rollback()
                    .await
                    .map_err(|e| AppError::Database(e.to_string()))?;
                return Err(AppError::from(
                    domain::scheduler::errors::SchedulerError::CapacityExceeded,
                ));
            }
        }

        // Insert booking
        sqlx::query(
            r#"
            INSERT INTO bookings (id, start_at, end_at, status, metadata, created_at, updated_at)
            VALUES ($1, $2, $3, 'confirmed', $4, $5, $6)
            "#,
        )
        .bind(booking.id)
        .bind(booking.start_at)
        .bind(booking.end_at)
        .bind(&booking.metadata)
        .bind(booking.created_at)
        .bind(booking.updated_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        // Insert booking_resources
        for &resource_id in &booking.resource_ids {
            sqlx::query(
                "INSERT INTO booking_resources (booking_id, resource_id, start_at, end_at) VALUES ($1, $2, $3, $4)",
            )
            .bind(booking.id)
            .bind(resource_id)
            .bind(booking.start_at)
            .bind(booking.end_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    async fn cancel(&self, booking: &Booking) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE bookings SET status = 'cancelled', updated_at = NOW() WHERE id = $1",
        )
        .bind(booking.id)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Booking>, AppError> {
        let row = sqlx::query_as::<_, BookingRow>(
            "SELECT id, start_at, end_at, status, metadata, created_at, updated_at FROM bookings WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        let Some(row) = row else { return Ok(None) };
        let resource_ids = load_resource_ids(&self.pool, row.id).await?;
        Ok(Some(row_to_booking(row, resource_ids)))
    }

    async fn find_overlapping_confirmed(
        &self,
        resource_id: Uuid,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Booking>, AppError> {
        let rows = sqlx::query_as::<_, BookingRow>(
            r#"
            SELECT DISTINCT b.id, b.start_at, b.end_at, b.status, b.metadata, b.created_at, b.updated_at
            FROM bookings b
            JOIN booking_resources br ON br.booking_id = b.id
            WHERE br.resource_id = $1
              AND b.status = 'confirmed'
              AND br.start_at < $3
              AND br.end_at > $2
            ORDER BY b.start_at
            "#,
        )
        .bind(resource_id)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        let mut result = Vec::new();
        for row in rows {
            let ids = load_resource_ids(&self.pool, row.id).await?;
            result.push(row_to_booking(row, ids));
        }
        Ok(result)
    }

    async fn find_confirmed_in_period(
        &self,
        resource_id: Uuid,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<Booking>, AppError> {
        let rows = sqlx::query_as::<_, BookingRow>(
            r#"
            SELECT DISTINCT b.id, b.start_at, b.end_at, b.status, b.metadata, b.created_at, b.updated_at
            FROM bookings b
            JOIN booking_resources br ON br.booking_id = b.id
            WHERE br.resource_id = $1
              AND b.status = 'confirmed'
              AND b.start_at >= $2
              AND b.end_at <= $3
            ORDER BY b.start_at
            "#,
        )
        .bind(resource_id)
        .bind(from)
        .bind(until)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        let mut result = Vec::new();
        for row in rows {
            let ids = load_resource_ids(&self.pool, row.id).await?;
            result.push(row_to_booking(row, ids));
        }
        Ok(result)
    }
}
