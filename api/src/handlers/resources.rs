use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use application::scheduler::commands::create_resource::CreateResourceCommand;
use application::scheduler::commands::create_schedule_rule::CreateScheduleRuleCommand;
use application::scheduler::commands::delete_schedule_rule::DeleteScheduleRuleCommand;
use application::scheduler::commands::update_schedule_rule::UpdateScheduleRuleCommand;
use application::scheduler::dto::{AvailableSlot, ResourceDto, ScheduleIntervalDto};
use application::scheduler::queries::get_available_slots::GetAvailableSlotsQuery;
use application::scheduler::queries::get_resource::GetResourceQuery;
use application::scheduler::queries::get_resource_schedule::GetResourceScheduleQuery;
use application::scheduler::queries::list_resources::ListResourcesQuery;
use domain::scheduler::schedule_rule::{RecurrenceParams, RuleKind};

use crate::errors::ApiError;
use crate::state::AppState;

// ─── Resources ───────────────────────────────────────────────────────────────

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
        .create_resource
        .handle(CreateResourceCommand {
            name: body.name,
            parent_id: body.parent_id,
            max_concurrent_events: body.max_concurrent_events,
            inherits_parent_schedule: body.inherits_parent_schedule,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(dto)))
}

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
        .list_resources
        .handle(ListResourcesQuery { parent_id: params.parent_id })
        .await?;
    Ok(Json(list))
}

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
        .get_resource
        .handle(GetResourceQuery { resource_id })
        .await?;
    Ok(Json(dto))
}

// ─── Schedule ────────────────────────────────────────────────────────────────

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
        .get_resource_schedule
        .handle(GetResourceScheduleQuery {
            resource_id,
            from: params.from,
            until: params.until,
        })
        .await?;
    Ok(Json(schedule))
}

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

// ─── Schedule Rules ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ScheduleRuleBody {
    pub kind: RuleKind,
    pub recurrence: RecurrenceParams,
    pub priority: i32,
    pub effective_from: Option<NaiveDate>,
    pub effective_until: Option<NaiveDate>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ScheduleRuleResponse {
    pub id: Uuid,
    pub resource_id: Uuid,
    pub kind: String,
    pub priority: i32,
    pub effective_from: Option<NaiveDate>,
    pub effective_until: Option<NaiveDate>,
}

#[utoipa::path(
    post,
    path = "/resources/{id}/schedule-rules",
    params(("id" = Uuid, Path, description = "Resource UUID")),
    request_body = ScheduleRuleBody,
    responses(
        (status = 201, description = "Schedule rule created", body = ScheduleRuleResponse),
        (status = 404, description = "Resource not found"),
        (status = 409, description = "Rule conflicts with existing bookings"),
    ),
    tag = "Schedule Rules"
)]
pub async fn create_schedule_rule(
    State(state): State<AppState>,
    Path(resource_id): Path<Uuid>,
    Json(body): Json<ScheduleRuleBody>,
) -> Result<(StatusCode, Json<ScheduleRuleResponse>), ApiError> {
    let rule = state
        .create_schedule_rule
        .handle(CreateScheduleRuleCommand {
            resource_id,
            kind: body.kind,
            recurrence: body.recurrence,
            priority: body.priority,
            effective_from: body.effective_from,
            effective_until: body.effective_until,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ScheduleRuleResponse {
            id: rule.id,
            resource_id: rule.resource_id,
            kind: format!("{:?}", rule.kind).to_lowercase(),
            priority: rule.priority,
            effective_from: rule.effective_from,
            effective_until: rule.effective_until,
        }),
    ))
}

#[utoipa::path(
    put,
    path = "/resources/{id}/schedule-rules/{rule_id}",
    params(
        ("id" = Uuid, Path, description = "Resource UUID"),
        ("rule_id" = Uuid, Path, description = "Schedule rule UUID"),
    ),
    request_body = ScheduleRuleBody,
    responses(
        (status = 200, description = "Schedule rule updated", body = ScheduleRuleResponse),
        (status = 404, description = "Rule not found"),
        (status = 409, description = "Rule conflicts with existing bookings"),
    ),
    tag = "Schedule Rules"
)]
pub async fn update_schedule_rule(
    State(state): State<AppState>,
    Path((resource_id, rule_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<ScheduleRuleBody>,
) -> Result<Json<ScheduleRuleResponse>, ApiError> {
    let rule = state
        .update_schedule_rule
        .handle(UpdateScheduleRuleCommand {
            rule_id,
            resource_id,
            kind: body.kind,
            recurrence: body.recurrence,
            priority: body.priority,
            effective_from: body.effective_from,
            effective_until: body.effective_until,
        })
        .await?;
    Ok(Json(ScheduleRuleResponse {
        id: rule.id,
        resource_id: rule.resource_id,
        kind: format!("{:?}", rule.kind).to_lowercase(),
        priority: rule.priority,
        effective_from: rule.effective_from,
        effective_until: rule.effective_until,
    }))
}

#[utoipa::path(
    delete,
    path = "/resources/{id}/schedule-rules/{rule_id}",
    params(
        ("id" = Uuid, Path, description = "Resource UUID"),
        ("rule_id" = Uuid, Path, description = "Schedule rule UUID"),
    ),
    responses(
        (status = 204, description = "Schedule rule deleted"),
        (status = 404, description = "Rule not found"),
        (status = 409, description = "Deletion conflicts with existing bookings"),
    ),
    tag = "Schedule Rules"
)]
pub async fn delete_schedule_rule(
    State(state): State<AppState>,
    Path((resource_id, rule_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    state
        .delete_schedule_rule
        .handle(DeleteScheduleRuleCommand { rule_id, resource_id })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
