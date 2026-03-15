use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::NaiveDate;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use application::scheduler::dto::AvailableSlot;
use application::scheduler::queries::get_available_slots::GetAvailableSlotsQuery;
use crate::errors::ApiError;
use crate::state::AppState;

#[derive(Debug, Deserialize, IntoParams)]
pub struct AvailabilityParams {
    /// Start date (inclusive)
    pub from: NaiveDate,
    /// End date (inclusive)
    pub until: NaiveDate,
    /// Slice into fixed-duration slots of this many minutes (optional)
    pub duration_minutes: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/resources/{id}/availability",
    params(
        ("id" = Uuid, Path, description = "Resource UUID"),
        AvailabilityParams,
    ),
    responses(
        (status = 200, description = "Available time slots", body = Vec<AvailableSlot>),
        (status = 404, description = "Resource not found"),
    ),
    tag = "Resources"
)]
pub async fn get_available_slots(
    State(state): State<AppState>,
    Path(resource_id): Path<Uuid>,
    Query(params): Query<AvailabilityParams>,
) -> Result<Json<Vec<AvailableSlot>>, ApiError> {
    let slots = state
        .resources
        .get_available_slots
        .handle(GetAvailableSlotsQuery {
            resource_id,
            from: params.from,
            until: params.until,
            duration_minutes: params.duration_minutes,
        })
        .await?;
    Ok(Json(slots))
}
