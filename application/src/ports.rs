use async_trait::async_trait;
use domain::events::DomainEvent;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::errors::AppError;

// ---- Event Bus ----

#[async_trait]
pub trait EventPublisher: Send + Sync {
    async fn publish(&self, event: &DomainEvent) -> Result<(), AppError>;
    async fn publish_many(&self, events: &[DomainEvent]) -> Result<(), AppError>;
}

// ---- Domain Event Handler ----

#[async_trait]
pub trait DomainEventHandler: Send + Sync {
    async fn handle(&self, event: &DomainEvent) -> Result<(), AppError>;
}

// ---- Event Dispatcher ----

#[async_trait]
pub trait EventDispatcher: Send + Sync {
    async fn dispatch(&self, events: &[DomainEvent]) -> Result<(), AppError>;
}

// ---- Cache ----

#[async_trait]
pub trait CachePort: Send + Sync {
    async fn get_raw(&self, key: &str) -> Result<Option<Vec<u8>>, AppError>;
    async fn set_raw(&self, key: &str, value: &[u8], ttl_secs: u64) -> Result<(), AppError>;
    async fn delete(&self, key: &str) -> Result<(), AppError>;
    async fn delete_pattern(&self, pattern: &str) -> Result<(), AppError>;
}

#[async_trait]
pub trait CachePortExt: CachePort {
    async fn get<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>, AppError> {
        match self.get_raw(key).await? {
            Some(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| AppError::Cache(format!("Deserialization error: {e}"))),
            None => Ok(None),
        }
    }

    async fn set<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl_secs: u64,
    ) -> Result<(), AppError> {
        let bytes = serde_json::to_vec(value)
            .map_err(|e| AppError::Cache(format!("Serialization error: {e}")))?;
        self.set_raw(key, &bytes, ttl_secs).await
    }
}

impl<C: CachePort + ?Sized> CachePortExt for C {}
