use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::NaiveDate;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use application::scheduler::dto::ScheduleIntervalDto;
use application::scheduler::queries::get_resource_schedule::GetResourceScheduleQuery;
use crate::errors::ApiError;
use crate::state::AppState;

#[derive(Debug, Deserialize, IntoParams)]
pub struct DateRangeParams {
    /// Start date (inclusive), e.g. 2026-03-14
    pub from: NaiveDate,
    /// End date (inclusive)
    pub until: NaiveDate,
}

#[utoipa::path(
    get,
    path = "/resources/{id}/schedule",
    params(
        ("id" = Uuid, Path, description = "Resource UUID"),
        DateRangeParams,
    ),
    responses(
        (status = 200, description = "Effective schedule intervals", body = Vec<ScheduleIntervalDto>),
        (status = 404, description = "Resource not found"),
    ),
    tag = "Resources"
)]
pub async fn get_resource_schedule(
    State(state): State<AppState>,
    Path(resource_id): Path<Uuid>,
    Query(params): Query<DateRangeParams>,
) -> Result<Json<Vec<ScheduleIntervalDto>>, ApiError> {
    let schedule = state
        .resources
        .get_schedule
        .handle(GetResourceScheduleQuery {
            resource_id,
            from: params.from,
            until: params.until,
        })
        .await?;
    Ok(Json(schedule))
}
