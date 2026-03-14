use domain::errors::DomainError;
use domain::scheduler::errors::SchedulerError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Event bus error: {0}")]
    EventBus(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<SchedulerError> for AppError {
    fn from(e: SchedulerError) -> Self {
        AppError::Domain(DomainError::Scheduler(e))
    }
}
