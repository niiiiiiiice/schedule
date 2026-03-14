use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use domain::errors::DomainError;
use domain::events::DomainEvent;
use domain::scheduler::events::SchedulerEvent;
use domain::scheduler::Resource;

use crate::errors::AppError;
use crate::ports::EventDispatcher;
use crate::scheduler::dto::ResourceDto;
use crate::scheduler::ports::ResourceRepository;

pub struct CreateResourceCommand {
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub max_concurrent_events: i32,
    pub inherits_parent_schedule: bool,
}

pub struct CreateResourceHandler {
    repo: Arc<dyn ResourceRepository>,
    dispatcher: Arc<dyn EventDispatcher>,
}

impl CreateResourceHandler {
    pub fn new(repo: Arc<dyn ResourceRepository>, dispatcher: Arc<dyn EventDispatcher>) -> Self {
        Self { repo, dispatcher }
    }

    pub async fn handle(&self, cmd: CreateResourceCommand) -> Result<ResourceDto, AppError> {
        if let Some(parent_id) = cmd.parent_id {
            if self.repo.find_by_id(parent_id).await?.is_none() {
                return Err(AppError::Domain(DomainError::NotFound(format!(
                    "Parent resource {parent_id}"
                ))));
            }
        }

        let resource = Resource::new(
            cmd.name,
            cmd.parent_id,
            cmd.max_concurrent_events,
            cmd.inherits_parent_schedule,
        )?;

        self.repo.save(&resource).await?;

        self.dispatcher
            .dispatch(&[DomainEvent::Scheduler(SchedulerEvent::ResourceCreated {
                resource_id: resource.id,
                name: resource.name.clone(),
                created_at: Utc::now(),
            })])
            .await?;

        Ok(ResourceDto::from(resource))
    }
}
