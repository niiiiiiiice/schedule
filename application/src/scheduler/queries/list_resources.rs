use std::sync::Arc;

use uuid::Uuid;

use crate::errors::AppError;
use crate::scheduler::dto::ResourceDto;
use crate::scheduler::ports::ResourceRepository;

pub struct ListResourcesQuery {
    /// If set, returns only direct children of this resource.
    pub parent_id: Option<Uuid>,
}

pub struct ListResourcesHandler {
    repo: Arc<dyn ResourceRepository>,
}

impl ListResourcesHandler {
    pub fn new(repo: Arc<dyn ResourceRepository>) -> Self {
        Self { repo }
    }

    pub async fn handle(&self, query: ListResourcesQuery) -> Result<Vec<ResourceDto>, AppError> {
        let resources = match query.parent_id {
            Some(pid) => self.repo.find_children(pid).await?,
            None => self.repo.list_all().await?,
        };
        Ok(resources.into_iter().map(ResourceDto::from).collect())
    }
}
