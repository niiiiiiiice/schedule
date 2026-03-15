use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use application::scheduler::commands::create_schedule_rule::CreateScheduleRuleCommand;
use application::scheduler::commands::delete_schedule_rule::DeleteScheduleRuleCommand;
use application::scheduler::commands::update_schedule_rule::UpdateScheduleRuleCommand;
use domain::scheduler::schedule_rule::{RecurrenceParams, RuleKind};
use domain::scheduler::ScheduleRule;
use crate::errors::ApiError;
use crate::state::AppState;

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

impl From<ScheduleRule> for ScheduleRuleResponse {
    fn from(rule: ScheduleRule) -> Self {
        Self {
            id: rule.id,
            resource_id: rule.resource_id,
            kind: format!("{:?}", rule.kind).to_lowercase(),
            priority: rule.priority,
            effective_from: rule.effective_from,
            effective_until: rule.effective_until,
        }
    }
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
        .resources
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
    Ok((StatusCode::CREATED, Json(rule.into())))
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
        .resources
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
    Ok(Json(rule.into()))
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
        .resources
        .delete_schedule_rule
        .handle(DeleteScheduleRuleCommand { rule_id, resource_id })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
