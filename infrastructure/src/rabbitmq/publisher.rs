use async_trait::async_trait;
use lapin::{
    options::{BasicPublishOptions, ExchangeDeclareOptions},
    types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};
use tracing::{info};

use application::errors::AppError;
use application::ports::EventPublisher;
use domain::events::DomainEvent;

// ============================================================
// RabbitMQ Publisher — реализация EventPublisher.
// Topic exchange: routing key = "task.created", "task.status_changed"
// В C# аналог: MassTransit IBus.Publish<T>() / RawRabbit.
// ============================================================

const EXCHANGE_NAME: &str = "domain_events";

pub struct RabbitMqPublisher {
    channel: Channel,
}

impl RabbitMqPublisher {
    pub async fn new(amqp_url: &str) -> Result<Self, AppError> {
        let conn = Connection::connect(amqp_url, ConnectionProperties::default())
            .await
            .map_err(|e| AppError::EventBus(format!("RabbitMQ connection failed: {e}")))?;

        let channel = conn
            .create_channel()
            .await
            .map_err(|e| AppError::EventBus(format!("RabbitMQ channel creation failed: {e}")))?;

        // Объявляем topic exchange
        channel
            .exchange_declare(
                EXCHANGE_NAME.into(),
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| AppError::EventBus(format!("Exchange declare failed: {e}")))?;

        info!("RabbitMQ publisher connected, exchange '{EXCHANGE_NAME}' declared");

        Ok(Self { channel })
    }
}

#[async_trait]
impl EventPublisher for RabbitMqPublisher {
    async fn publish(&self, event: &DomainEvent) -> Result<(), AppError> {
        let routing_key = event.routing_key();
        let payload = serde_json::to_vec(event)
            .map_err(|e| AppError::EventBus(format!("Serialization error: {e}")))?;

        self.channel
            .basic_publish(
                EXCHANGE_NAME.into(),
                routing_key.into(),
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default()
                    .with_content_type("application/json".into())
                    .with_delivery_mode(2), // persistent
            )
            .await
            .map_err(|e| AppError::EventBus(format!("Publish failed: {e}")))?
            .await
            .map_err(|e| AppError::EventBus(format!("Publish confirm failed: {e}")))?;

        info!(routing_key = routing_key, "Event published");

        Ok(())
    }

    async fn publish_many(&self, events: &[DomainEvent]) -> Result<(), AppError> {
        for event in events {
            self.publish(event).await?;
        }
        Ok(())
    }
}
