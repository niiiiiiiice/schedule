use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use uuid::Uuid;

use domain::scheduler::{Booking, Resource, ScheduleRule, TimeInterval};

use crate::errors::AppError;

// ---- Resource ----

#[async_trait]
pub trait ResourceRepository: Send + Sync {
    async fn save(&self, resource: &Resource) -> Result<(), AppError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Resource>, AppError>;
    async fn find_children(&self, parent_id: Uuid) -> Result<Vec<Resource>, AppError>;
    /// Ancestors ordered nearest-to-root.
    async fn find_ancestors(&self, id: Uuid) -> Result<Vec<Resource>, AppError>;
    async fn list_all(&self) -> Result<Vec<Resource>, AppError>;
}

// ---- Schedule Rules ----

#[async_trait]
pub trait ScheduleRuleRepository: Send + Sync {
    async fn save(&self, rule: &ScheduleRule) -> Result<(), AppError>;
    async fn update(&self, rule: &ScheduleRule) -> Result<(), AppError>;
    async fn delete(&self, id: Uuid) -> Result<(), AppError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<ScheduleRule>, AppError>;
    async fn find_by_resource(&self, resource_id: Uuid) -> Result<Vec<ScheduleRule>, AppError>;
}

// ---- Effective Intervals (materialized schedule) ----

#[derive(Debug, Clone)]
pub struct EffectiveIntervalRow {
    pub date: NaiveDate,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub available_capacity: i32,
}

impl EffectiveIntervalRow {
    pub fn to_time_interval(&self) -> TimeInterval {
        TimeInterval::new(self.start_time, self.end_time)
            .expect("DB interval must always be valid")
    }
}

#[async_trait]
pub trait EffectiveIntervalStore: Send + Sync {
    async fn get_for_resource_and_range(
        &self,
        resource_id: Uuid,
        from: NaiveDate,
        until: NaiveDate,
    ) -> Result<Vec<EffectiveIntervalRow>, AppError>;

    /// Deletes all intervals for resource in [from, until] then inserts new ones.
    async fn replace_for_resource(
        &self,
        resource_id: Uuid,
        from: NaiveDate,
        until: NaiveDate,
        intervals: &[(NaiveDate, TimeInterval, i32)],
    ) -> Result<(), AppError>;

    async fn delete_for_resource(&self, resource_id: Uuid) -> Result<(), AppError>;
}

// ---- Bookings ----

#[async_trait]
pub trait BookingRepository: Send + Sync {
    /// Atomically creates a booking:
    /// - Acquires pg_advisory_xact_lock per resource (sorted ascending)
    /// - Checks confirmed booking count < max_concurrent_events for each resource
    /// - Inserts booking + booking_resources
    async fn create_atomic(
        &self,
        booking: &Booking,
        resource_max_concurrent: &[(Uuid, i32)],
    ) -> Result<(), AppError>;

    async fn cancel(&self, booking: &Booking) -> Result<(), AppError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Booking>, AppError>;

    /// All confirmed bookings overlapping [start, end) for a resource.
    async fn find_overlapping_confirmed(
        &self,
        resource_id: Uuid,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Booking>, AppError>;

    /// All confirmed bookings within a period for a resource.
    async fn find_confirmed_in_period(
        &self,
        resource_id: Uuid,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<Booking>, AppError>;
}
