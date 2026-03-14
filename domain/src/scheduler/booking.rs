use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use super::errors::SchedulerError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookingStatus {
    Confirmed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct Booking {
    pub id: Uuid,
    pub resource_ids: Vec<Uuid>,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub status: BookingStatus,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Booking {
    pub fn new(
        resource_ids: Vec<Uuid>,
        start_at: DateTime<Utc>,
        end_at: DateTime<Utc>,
        metadata: Option<Value>,
    ) -> Result<Self, SchedulerError> {
        if resource_ids.is_empty() {
            return Err(SchedulerError::Validation(
                "At least one resource is required".into(),
            ));
        }
        if start_at >= end_at {
            return Err(SchedulerError::InvalidTimeRange);
        }
        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            resource_ids,
            start_at,
            end_at,
            status: BookingStatus::Confirmed,
            metadata,
            created_at: now,
            updated_at: now,
        })
    }

    /// Восстановление из БД.
    pub fn from_raw(
        id: Uuid,
        resource_ids: Vec<Uuid>,
        start_at: DateTime<Utc>,
        end_at: DateTime<Utc>,
        status: BookingStatus,
        metadata: Option<Value>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            resource_ids,
            start_at,
            end_at,
            status,
            metadata,
            created_at,
            updated_at,
        }
    }

    pub fn cancel(&mut self) -> Result<(), SchedulerError> {
        if self.status == BookingStatus::Cancelled {
            return Err(SchedulerError::Validation(
                "Booking is already cancelled".into(),
            ));
        }
        self.status = BookingStatus::Cancelled;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn is_confirmed(&self) -> bool {
        self.status == BookingStatus::Confirmed
    }
}
