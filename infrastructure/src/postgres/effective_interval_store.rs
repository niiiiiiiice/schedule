use async_trait::async_trait;
use chrono::{NaiveDate, NaiveTime};
use sqlx::PgPool;
use uuid::Uuid;

use application::errors::AppError;
use application::scheduler::ports::{EffectiveIntervalRow, EffectiveIntervalStore};
use domain::scheduler::TimeInterval;

pub struct PgEffectiveIntervalStore {
    pool: PgPool,
}

impl PgEffectiveIntervalStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct IntervalRow {
    date: NaiveDate,
    start_time: NaiveTime,
    end_time: NaiveTime,
    available_capacity: i32,
}

#[async_trait]
impl EffectiveIntervalStore for PgEffectiveIntervalStore {
    async fn get_for_resource_and_range(
        &self,
        resource_id: Uuid,
        from: NaiveDate,
        until: NaiveDate,
    ) -> Result<Vec<EffectiveIntervalRow>, AppError> {
        let rows = sqlx::query_as::<_, IntervalRow>(
            r#"
            SELECT date, start_time, end_time, available_capacity
            FROM effective_intervals
            WHERE resource_id = $1 AND date >= $2 AND date <= $3
            ORDER BY date, start_time
            "#,
        )
        .bind(resource_id)
        .bind(from)
        .bind(until)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| EffectiveIntervalRow {
                date: r.date,
                start_time: r.start_time,
                end_time: r.end_time,
                available_capacity: r.available_capacity,
            })
            .collect())
    }

    async fn replace_for_resource(
        &self,
        resource_id: Uuid,
        from: NaiveDate,
        until: NaiveDate,
        intervals: &[(NaiveDate, TimeInterval, i32)],
    ) -> Result<(), AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        sqlx::query(
            "DELETE FROM effective_intervals WHERE resource_id = $1 AND date >= $2 AND date <= $3",
        )
        .bind(resource_id)
        .bind(from)
        .bind(until)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        for (date, iv, capacity) in intervals {
            sqlx::query(
                r#"
                INSERT INTO effective_intervals (resource_id, date, start_time, end_time, available_capacity)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (resource_id, date, start_time) DO UPDATE
                    SET end_time = EXCLUDED.end_time,
                        available_capacity = EXCLUDED.available_capacity,
                        computed_at = NOW()
                "#,
            )
            .bind(resource_id)
            .bind(date)
            .bind(iv.start)
            .bind(iv.end)
            .bind(capacity)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    async fn delete_for_resource(&self, resource_id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM effective_intervals WHERE resource_id = $1")
            .bind(resource_id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }
}
