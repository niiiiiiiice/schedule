use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

use application::errors::AppError;
use application::scheduler::ports::ScheduleRuleRepository;
use domain::scheduler::schedule_rule::{RecurrenceParams, RuleKind};
use domain::scheduler::ScheduleRule;

pub struct PgScheduleRuleRepository {
    pool: PgPool,
}

impl PgScheduleRuleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ScheduleRuleRow {
    id: Uuid,
    resource_id: Uuid,
    rule_kind: String,
    #[allow(dead_code)]
    recurrence_type: String,
    priority: i32,
    effective_from: Option<NaiveDate>,
    effective_until: Option<NaiveDate>,
    parameters: serde_json::Value,
}

fn row_to_rule(row: ScheduleRuleRow) -> Result<ScheduleRule, AppError> {
    let kind = match row.rule_kind.as_str() {
        "availability" => RuleKind::Availability,
        "break" => RuleKind::Break,
        other => return Err(AppError::Database(format!("Unknown rule_kind: {other}"))),
    };
    let recurrence: RecurrenceParams = serde_json::from_value(row.parameters)
        .map_err(|e| AppError::Database(format!("Failed to parse rule parameters: {e}")))?;

    Ok(ScheduleRule {
        id: row.id,
        resource_id: row.resource_id,
        kind,
        recurrence,
        priority: row.priority,
        effective_from: row.effective_from,
        effective_until: row.effective_until,
    })
}

fn recurrence_type_str(r: &RecurrenceParams) -> &'static str {
    match r {
        RecurrenceParams::Once { .. } => "once",
        RecurrenceParams::Daily { .. } => "daily",
        RecurrenceParams::Weekly { .. } => "weekly",
        RecurrenceParams::Custom { .. } => "custom",
    }
}

#[async_trait]
impl ScheduleRuleRepository for PgScheduleRuleRepository {
    async fn save(&self, rule: &ScheduleRule) -> Result<(), AppError> {
        let kind_str = match rule.kind {
            RuleKind::Availability => "availability",
            RuleKind::Break => "break",
        };
        let recurrence_type = recurrence_type_str(&rule.recurrence);
        let parameters = serde_json::to_value(&rule.recurrence)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO schedule_rules
                (id, resource_id, rule_kind, recurrence_type, priority, effective_from, effective_until, parameters)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(rule.id)
        .bind(rule.resource_id)
        .bind(kind_str)
        .bind(recurrence_type)
        .bind(rule.priority)
        .bind(rule.effective_from)
        .bind(rule.effective_until)
        .bind(parameters)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    async fn update(&self, rule: &ScheduleRule) -> Result<(), AppError> {
        let kind_str = match rule.kind {
            RuleKind::Availability => "availability",
            RuleKind::Break => "break",
        };
        let recurrence_type = recurrence_type_str(&rule.recurrence);
        let parameters = serde_json::to_value(&rule.recurrence)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        sqlx::query(
            r#"
            UPDATE schedule_rules
            SET rule_kind = $2, recurrence_type = $3, priority = $4,
                effective_from = $5, effective_until = $6, parameters = $7,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(rule.id)
        .bind(kind_str)
        .bind(recurrence_type)
        .bind(rule.priority)
        .bind(rule.effective_from)
        .bind(rule.effective_until)
        .bind(parameters)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM schedule_rules WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<ScheduleRule>, AppError> {
        let row = sqlx::query_as::<_, ScheduleRuleRow>(
            "SELECT id, resource_id, rule_kind, recurrence_type, priority, effective_from, effective_until, parameters FROM schedule_rules WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        row.map(row_to_rule).transpose()
    }

    async fn find_by_resource(&self, resource_id: Uuid) -> Result<Vec<ScheduleRule>, AppError> {
        let rows = sqlx::query_as::<_, ScheduleRuleRow>(
            "SELECT id, resource_id, rule_kind, recurrence_type, priority, effective_from, effective_until, parameters FROM schedule_rules WHERE resource_id = $1 ORDER BY priority DESC",
        )
        .bind(resource_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        rows.into_iter().map(row_to_rule).collect()
    }
}
