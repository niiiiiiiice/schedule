use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use application::scheduler::dto::ResourceDto;
use application::scheduler::queries::list_resources::ListResourcesQuery;
use crate::errors::ApiError;
use crate::state::AppState;

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListResourcesParams {
    /// Filter by parent resource ID (returns root resources if omitted)
    pub parent_id: Option<Uuid>,
}

#[utoipa::path(
    get,
    path = "/resources",
    params(ListResourcesParams),
    responses(
        (status = 200, description = "List of resources", body = Vec<ResourceDto>),
    ),
    tag = "Resources"
)]
pub async fn list_resources(
    State(state): State<AppState>,
    Query(params): Query<ListResourcesParams>,
) -> Result<Json<Vec<ResourceDto>>, ApiError> {
    let list = state
        .resources
        .list
        .handle(ListResourcesQuery { parent_id: params.parent_id })
        .await?;
    Ok(Json(list))
}
