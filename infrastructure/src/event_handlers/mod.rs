use std::sync::Arc;

use async_trait::async_trait;
use domain::events::DomainEvent;

use application::errors::AppError;
use application::ports::{DomainEventHandler, EventPublisher};

pub struct RabbitMqEventHandler {
    publisher: Arc<dyn EventPublisher>,
}

impl RabbitMqEventHandler {
    pub fn new(publisher: Arc<dyn EventPublisher>) -> Self {
        Self { publisher }
    }
}

#[async_trait]
impl DomainEventHandler for RabbitMqEventHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<(), AppError> {
        self.publisher.publish(event).await
    }
}
