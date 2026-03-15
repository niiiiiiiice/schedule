use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use application::scheduler::dto::ResourceDto;
use application::scheduler::queries::get_resource::GetResourceQuery;
use crate::errors::ApiError;
use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/resources/{id}",
    params(("id" = Uuid, Path, description = "Resource UUID")),
    responses(
        (status = 200, description = "Resource details", body = ResourceDto),
        (status = 404, description = "Resource not found"),
    ),
    tag = "Resources"
)]
pub async fn get_resource(
    State(state): State<AppState>,
    Path(resource_id): Path<Uuid>,
) -> Result<Json<ResourceDto>, ApiError> {
    let dto = state
        .resources
        .get
        .handle(GetResourceQuery { resource_id })
        .await?;
    Ok(Json(dto))
}
