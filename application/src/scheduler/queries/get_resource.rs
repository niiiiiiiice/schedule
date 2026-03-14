use std::sync::Arc;

use uuid::Uuid;

use domain::errors::DomainError;

use crate::errors::AppError;
use crate::scheduler::dto::ResourceDto;
use crate::scheduler::ports::ResourceRepository;

pub struct GetResourceQuery {
    pub resource_id: Uuid,
}

pub struct GetResourceHandler {
    repo: Arc<dyn ResourceRepository>,
}

impl GetResourceHandler {
    pub fn new(repo: Arc<dyn ResourceRepository>) -> Self {
        Self { repo }
    }

    pub async fn handle(&self, query: GetResourceQuery) -> Result<ResourceDto, AppError> {
        self.repo
            .find_by_id(query.resource_id)
            .await?
            .map(ResourceDto::from)
            .ok_or_else(|| {
                AppError::Domain(DomainError::NotFound(format!(
                    "Resource {}",
                    query.resource_id
                )))
            })
    }
}
