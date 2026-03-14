use std::sync::Arc;

use async_trait::async_trait;
use domain::events::DomainEvent;

use crate::errors::AppError;
use crate::ports::{DomainEventHandler, EventDispatcher};

// ============================================================
// InProcessEventDispatcher — синхронная in-process шина событий.
// Вызывает всех зарегистрированных подписчиков последовательно.
// В C# аналог: MediatR IPublisher с несколькими INotificationHandler.
// ============================================================

pub struct InProcessEventDispatcher {
    handlers: Vec<Arc<dyn DomainEventHandler>>,
}

impl InProcessEventDispatcher {
    pub fn new(handlers: Vec<Arc<dyn DomainEventHandler>>) -> Self {
        Self { handlers }
    }
}

#[async_trait]
impl EventDispatcher for InProcessEventDispatcher {
    async fn dispatch(&self, events: &[DomainEvent]) -> Result<(), AppError> {
        for event in events {
            for handler in &self.handlers {
                handler.handle(event).await?;
            }
        }
        Ok(())
    }
}
