use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use application::scheduler::commands::create_resource::CreateResourceCommand;
use application::scheduler::dto::ResourceDto;
use crate::errors::ApiError;
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateResourceBody {
    /// Human-readable name of the resource
    pub name: String,
    /// Optional parent resource UUID (for hierarchy)
    pub parent_id: Option<Uuid>,
    /// Maximum simultaneous bookings (default: 1)
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_events: i32,
    /// Whether this resource inherits its parent's schedule
    #[serde(default)]
    pub inherits_parent_schedule: bool,
}

fn default_max_concurrent() -> i32 {
    1
}

#[utoipa::path(
    post,
    path = "/resources",
    request_body = CreateResourceBody,
    responses(
        (status = 201, description = "Resource created", body = ResourceDto),
        (status = 400, description = "Validation error"),
        (status = 404, description = "Parent resource not found"),
    ),
    tag = "Resources"
)]
pub async fn create_resource(
    State(state): State<AppState>,
    Json(body): Json<CreateResourceBody>,
) -> Result<(StatusCode, Json<ResourceDto>), ApiError> {
    let dto = state
        .resources
        .create
        .handle(CreateResourceCommand {
            name: body.name,
            parent_id: body.parent_id,
            max_concurrent_events: body.max_concurrent_events,
            inherits_parent_schedule: body.inherits_parent_schedule,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(dto)))
}
