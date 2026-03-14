use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::errors::SchedulerError;

#[derive(Debug, Clone)]
pub struct Resource {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub max_concurrent_events: i32,
    pub inherits_parent_schedule: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Resource {
    pub fn new(
        name: String,
        parent_id: Option<Uuid>,
        max_concurrent_events: i32,
        inherits_parent_schedule: bool,
    ) -> Result<Self, SchedulerError> {
        let name = name.trim().to_string();
        if name.is_empty() || name.len() > 200 {
            return Err(SchedulerError::Validation(
                "Name must be 1-200 characters".into(),
            ));
        }
        if max_concurrent_events < 1 {
            return Err(SchedulerError::Validation(
                "max_concurrent_events must be at least 1".into(),
            ));
        }
        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            name,
            parent_id,
            max_concurrent_events,
            inherits_parent_schedule,
            created_at: now,
            updated_at: now,
        })
    }

    /// Восстановление из БД без валидации.
    pub fn from_raw(
        id: Uuid,
        name: String,
        parent_id: Option<Uuid>,
        max_concurrent_events: i32,
        inherits_parent_schedule: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            name,
            parent_id,
            max_concurrent_events,
            inherits_parent_schedule,
            created_at,
            updated_at,
        }
    }
}
