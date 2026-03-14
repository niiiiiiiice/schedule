use crate::scheduler::errors::SchedulerError;

#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error(transparent)]
    Scheduler(#[from] SchedulerError),
}
