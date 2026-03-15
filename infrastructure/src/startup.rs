use std::sync::Arc;

use tracing::info;

use application::dispatcher::InProcessEventDispatcher;
use crate::event_handlers::RabbitMqEventHandler;
use crate::postgres;
use crate::rabbitmq::RabbitMqPublisher;
use crate::redis::RedisCache;
use crate::{
    PgBookingRepository, PgEffectiveIntervalStore, PgResourceRepository, PgScheduleRuleRepository,
};

/// All infrastructure dependencies wired up and ready for handler construction.
pub struct Infrastructure {
    pub resource_repo: Arc<PgResourceRepository>,
    pub rule_repo: Arc<PgScheduleRuleRepository>,
    pub interval_store: Arc<PgEffectiveIntervalStore>,
    pub booking_repo: Arc<PgBookingRepository>,
    pub dispatcher: Arc<InProcessEventDispatcher>,
}

/// Environment-driven configuration with sensible defaults.
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub amqp_url: String,
    pub bind_addr: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:1@localhost:5432/scheduler_db".into()),
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".into()),
            amqp_url: std::env::var("AMQP_URL")
                .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/%2f".into()),
            bind_addr: std::env::var("BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:3000".into()),
        }
    }
}

/// Connect to PostgreSQL, Redis, RabbitMQ and build repositories + event dispatcher.
pub async fn init(config: &Config) -> anyhow::Result<Infrastructure> {
    // ── PostgreSQL ────────────────────────────────────────────────────────
    info!("Connecting to PostgreSQL...");
    let pg_pool = postgres::create_pool(&config.database_url).await?;
    postgres::run_migrations(&pg_pool).await?;
    info!("PostgreSQL connected, migrations applied");

    // ── Redis ─────────────────────────────────────────────────────────────
    info!("Connecting to Redis...");
    let _redis_cache = RedisCache::new(&config.redis_url).await?;
    info!("Redis connected");

    // ── RabbitMQ ──────────────────────────────────────────────────────────
    info!("Connecting to RabbitMQ...");
    let rabbit_publisher = Arc::new(RabbitMqPublisher::new(&config.amqp_url).await?);
    info!("RabbitMQ connected");

    // ── Repositories ──────────────────────────────────────────────────────
    let resource_repo = Arc::new(PgResourceRepository::new(pg_pool.clone()));
    let rule_repo = Arc::new(PgScheduleRuleRepository::new(pg_pool.clone()));
    let interval_store = Arc::new(PgEffectiveIntervalStore::new(pg_pool.clone()));
    let booking_repo = Arc::new(PgBookingRepository::new(pg_pool.clone()));

    // ── Event dispatcher ──────────────────────────────────────────────────
    let rabbit_handler = Arc::new(RabbitMqEventHandler::new(rabbit_publisher));
    let dispatcher = Arc::new(InProcessEventDispatcher::new(vec![rabbit_handler]));

    Ok(Infrastructure {
        resource_repo,
        rule_repo,
        interval_store,
        booking_repo,
        dispatcher,
    })
}