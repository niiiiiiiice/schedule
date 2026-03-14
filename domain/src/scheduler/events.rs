use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SchedulerEvent {
    ResourceCreated {
        resource_id: Uuid,
        name: String,
        created_at: DateTime<Utc>,
    },
    BookingCreated {
        booking_id: Uuid,
        resource_ids: Vec<Uuid>,
        start_at: DateTime<Utc>,
        end_at: DateTime<Utc>,
        created_at: DateTime<Utc>,
    },
    BookingCancelled {
        booking_id: Uuid,
        cancelled_at: DateTime<Utc>,
    },
    ScheduleRuleCreated {
        resource_id: Uuid,
        rule_id: Uuid,
        created_at: DateTime<Utc>,
    },
    ScheduleRuleUpdated {
        resource_id: Uuid,
        rule_id: Uuid,
        updated_at: DateTime<Utc>,
    },
    ScheduleRuleDeleted {
        resource_id: Uuid,
        rule_id: Uuid,
        deleted_at: DateTime<Utc>,
    },
}

impl SchedulerEvent {
    pub fn routing_key(&self) -> &'static str {
        match self {
            SchedulerEvent::ResourceCreated { .. } => "scheduler.resource.created",
            SchedulerEvent::BookingCreated { .. } => "scheduler.booking.created",
            SchedulerEvent::BookingCancelled { .. } => "scheduler.booking.cancelled",
            SchedulerEvent::ScheduleRuleCreated { .. } => "scheduler.rule.created",
            SchedulerEvent::ScheduleRuleUpdated { .. } => "scheduler.rule.updated",
            SchedulerEvent::ScheduleRuleDeleted { .. } => "scheduler.rule.deleted",
        }
    }
}
