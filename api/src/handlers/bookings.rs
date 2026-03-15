use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use application::scheduler::commands::cancel_booking::CancelBookingCommand;
use application::scheduler::commands::create_booking::CreateBookingCommand;
use application::scheduler::dto::BookingDto;
use application::scheduler::queries::get_booking::GetBookingQuery;

use crate::errors::ApiError;
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateBookingBody {
    /// List of resource UUIDs to book simultaneously
    pub resource_ids: Vec<Uuid>,
    /// Booking start (UTC)
    pub start_at: DateTime<Utc>,
    /// Booking end (UTC)
    pub end_at: DateTime<Utc>,
    /// Arbitrary JSON metadata (optional)
    #[schema(value_type = Option<Object>)]
    pub metadata: Option<serde_json::Value>,
}

#[utoipa::path(
    post,
    path = "/bookings",
    request_body = CreateBookingBody,
    responses(
        (status = 201, description = "Booking created", body = BookingDto),
        (status = 404, description = "Resource not found"),
        (status = 409, description = "Capacity exceeded"),
        (status = 422, description = "Time slot not within resource schedule"),
    ),
    tag = "Bookings"
)]
pub async fn create_booking(
    State(state): State<AppState>,
    Json(body): Json<CreateBookingBody>,
) -> Result<(StatusCode, Json<BookingDto>), ApiError> {
    let dto = state
        .bookings
        .create
        .handle(CreateBookingCommand {
            resource_ids: body.resource_ids,
            start_at: body.start_at,
            end_at: body.end_at,
            metadata: body.metadata,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(dto)))
}

#[utoipa::path(
    get,
    path = "/bookings/{id}",
    params(("id" = Uuid, Path, description = "Booking UUID")),
    responses(
        (status = 200, description = "Booking details", body = BookingDto),
        (status = 404, description = "Booking not found"),
    ),
    tag = "Bookings"
)]
pub async fn get_booking(
    State(state): State<AppState>,
    Path(booking_id): Path<Uuid>,
) -> Result<Json<BookingDto>, ApiError> {
    let dto = state
        .bookings
        .get
        .handle(GetBookingQuery { booking_id })
        .await?;
    Ok(Json(dto))
}

#[utoipa::path(
    post,
    path = "/bookings/{id}/cancel",
    params(("id" = Uuid, Path, description = "Booking UUID")),
    responses(
        (status = 204, description = "Booking cancelled"),
        (status = 404, description = "Booking not found"),
    ),
    tag = "Bookings"
)]
pub async fn cancel_booking(
    State(state): State<AppState>,
    Path(booking_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .bookings
        .cancel
        .handle(CancelBookingCommand { booking_id })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
