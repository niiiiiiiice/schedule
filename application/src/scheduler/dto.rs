use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use domain::scheduler::booking::BookingStatus;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ResourceDto {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub max_concurrent_events: i32,
    pub inherits_parent_schedule: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<domain::scheduler::Resource> for ResourceDto {
    fn from(r: domain::scheduler::Resource) -> Self {
        Self {
            id: r.id,
            name: r.name,
            parent_id: r.parent_id,
            max_concurrent_events: r.max_concurrent_events,
            inherits_parent_schedule: r.inherits_parent_schedule,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AvailableSlot {
    pub date: NaiveDate,
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub remaining_capacity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ScheduleIntervalDto {
    pub date: NaiveDate,
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub available_capacity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BookingDto {
    pub id: Uuid,
    pub resource_ids: Vec<Uuid>,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub status: String,
    #[schema(value_type = Option<Object>)]
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<domain::scheduler::Booking> for BookingDto {
    fn from(b: domain::scheduler::Booking) -> Self {
        let status = match b.status {
            BookingStatus::Confirmed => "confirmed",
            BookingStatus::Cancelled => "cancelled",
        }
        .to_string();
        Self {
            id: b.id,
            resource_ids: b.resource_ids,
            start_at: b.start_at,
            end_at: b.end_at,
            status,
            metadata: b.metadata,
            created_at: b.created_at,
            updated_at: b.updated_at,
        }
    }
}
