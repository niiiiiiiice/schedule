use axum::routing::{get, post, put};
use axum::Router;

use crate::handlers::bookings as booking_handlers;
use crate::handlers::resources as resource_handlers;
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .merge(resource_routes())
        .merge(booking_routes())
        .with_state(state)
}

fn resource_routes() -> Router<AppState> {
    Router::new()
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
}

fn booking_routes() -> Router<AppState> {
    Router::new()
        .route("/bookings", post(booking_handlers::create_booking))
        .route("/bookings/{id}", get(booking_handlers::get_booking))
        .route(
            "/bookings/{id}/cancel",
            post(booking_handlers::cancel_booking),
        )
}

async fn health_check() -> &'static str {
    "OK"
}