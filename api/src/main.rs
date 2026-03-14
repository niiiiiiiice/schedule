use std::sync::Arc;

use axum::routing::{get, post, put};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use api::handlers::bookings as booking_handlers;
use api::handlers::resources as resource_handlers;
use api::openapi::ApiDoc;
use api::state::AppState;
use application::dispatcher::InProcessEventDispatcher;
use application::scheduler::commands::cancel_booking::CancelBookingHandler;
use application::scheduler::commands::create_booking::CreateBookingHandler;
use application::scheduler::commands::create_resource::CreateResourceHandler;
use application::scheduler::commands::create_schedule_rule::CreateScheduleRuleHandler;
use application::scheduler::commands::delete_schedule_rule::DeleteScheduleRuleHandler;
use application::scheduler::commands::update_schedule_rule::UpdateScheduleRuleHandler;
use application::scheduler::queries::get_available_slots::GetAvailableSlotsHandler;
use application::scheduler::queries::get_booking::GetBookingHandler;
use application::scheduler::queries::get_resource::GetResourceHandler;
use application::scheduler::queries::get_resource_schedule::GetResourceScheduleHandler;
use application::scheduler::queries::list_resources::ListResourcesHandler;
use infrastructure::event_handlers::RabbitMqEventHandler;
use infrastructure::postgres;
use infrastructure::rabbitmq::RabbitMqPublisher;
use infrastructure::redis::RedisCache;
use infrastructure::{
    PgBookingRepository, PgEffectiveIntervalStore, PgResourceRepository, PgScheduleRuleRepository,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api=debug,infrastructure=debug,application=debug".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1@localhost:5432/scheduler_db".into());
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
    let amqp_url = std::env::var("AMQP_URL")
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/%2f".into());
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());

    info!("Connecting to PostgreSQL...");
    let pg_pool = postgres::create_pool(&database_url).await?;
    postgres::run_migrations(&pg_pool).await?;
    info!("PostgreSQL connected, migrations applied");

    info!("Connecting to Redis...");
    let _redis_cache = RedisCache::new(&redis_url).await?;
    info!("Redis connected");

    info!("Connecting to RabbitMQ...");
    let rabbit_publisher = Arc::new(RabbitMqPublisher::new(&amqp_url).await?);
    info!("RabbitMQ connected");

    // Repositories
    let resource_repo = Arc::new(PgResourceRepository::new(pg_pool.clone()));
    let rule_repo = Arc::new(PgScheduleRuleRepository::new(pg_pool.clone()));
    let interval_store = Arc::new(PgEffectiveIntervalStore::new(pg_pool.clone()));
    let booking_repo = Arc::new(PgBookingRepository::new(pg_pool.clone()));

    // Event dispatcher
    let rabbit_handler = Arc::new(RabbitMqEventHandler::new(rabbit_publisher));
    let dispatcher = Arc::new(InProcessEventDispatcher::new(vec![rabbit_handler]));

    // Command handlers
    let create_resource_h = Arc::new(CreateResourceHandler::new(
        resource_repo.clone(),
        dispatcher.clone(),
    ));
    let create_schedule_rule_h = Arc::new(CreateScheduleRuleHandler::new(
        resource_repo.clone(),
        rule_repo.clone(),
        interval_store.clone(),
        booking_repo.clone(),
        dispatcher.clone(),
    ));
    let update_schedule_rule_h = Arc::new(UpdateScheduleRuleHandler::new(
        resource_repo.clone(),
        rule_repo.clone(),
        interval_store.clone(),
        booking_repo.clone(),
        dispatcher.clone(),
    ));
    let delete_schedule_rule_h = Arc::new(DeleteScheduleRuleHandler::new(
        resource_repo.clone(),
        rule_repo.clone(),
        interval_store.clone(),
        booking_repo.clone(),
        dispatcher.clone(),
    ));
    let create_booking_h = Arc::new(CreateBookingHandler::new(
        resource_repo.clone(),
        rule_repo.clone(),
        interval_store.clone(),
        booking_repo.clone(),
        dispatcher.clone(),
    ));
    let cancel_booking_h = Arc::new(CancelBookingHandler::new(
        booking_repo.clone(),
        dispatcher.clone(),
    ));

    // Query handlers
    let get_resource_h = Arc::new(GetResourceHandler::new(resource_repo.clone()));
    let list_resources_h = Arc::new(ListResourcesHandler::new(resource_repo.clone()));
    let get_resource_schedule_h = Arc::new(GetResourceScheduleHandler::new(
        resource_repo.clone(),
        rule_repo.clone(),
        interval_store.clone(),
    ));
    let get_available_slots_h = Arc::new(GetAvailableSlotsHandler::new(
        resource_repo.clone(),
        rule_repo.clone(),
        interval_store.clone(),
        booking_repo.clone(),
    ));
    let get_booking_h = Arc::new(GetBookingHandler::new(booking_repo.clone()));

    let state = AppState {
        create_resource: create_resource_h,
        get_resource: get_resource_h,
        list_resources: list_resources_h,
        get_resource_schedule: get_resource_schedule_h,
        get_available_slots: get_available_slots_h,
        create_schedule_rule: create_schedule_rule_h,
        update_schedule_rule: update_schedule_rule_h,
        delete_schedule_rule: delete_schedule_rule_h,
        create_booking: create_booking_h,
        get_booking: get_booking_h,
        cancel_booking: cancel_booking_h,
    };

    let api = Router::new()
        .route("/health", get(health_check))
        // Resources
        .route(
            "/resources",
            post(resource_handlers::create_resource).get(resource_handlers::list_resources),
        )
        .route("/resources/{id}", get(resource_handlers::get_resource))
        .route(
            "/resources/{id}/schedule",
            get(resource_handlers::get_resource_schedule),
        )
        .route(
            "/resources/{id}/availability",
            get(resource_handlers::get_available_slots),
        )
        .route(
            "/resources/{id}/schedule-rules",
            post(resource_handlers::create_schedule_rule),
        )
        .route(
            "/resources/{id}/schedule-rules/{rule_id}",
            put(resource_handlers::update_schedule_rule)
                .delete(resource_handlers::delete_schedule_rule),
        )
        // Bookings
        .route("/bookings", post(booking_handlers::create_booking))
        .route("/bookings/{id}", get(booking_handlers::get_booking))
        .route("/bookings/{id}/cancel", post(booking_handlers::cancel_booking))
        .with_state(state);

    let app = api
        .merge(Scalar::with_url("/scalar", ApiDoc::openapi()))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let display_addr = bind_addr.replace("0.0.0.0", "localhost");
    info!("Server starting on http://{display_addr}");
    info!("Scalar available on http://{display_addr}/scalar");
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}
