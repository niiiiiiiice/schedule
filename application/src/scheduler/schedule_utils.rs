use std::sync::Arc;

use chrono::NaiveDate;
use uuid::Uuid;

use domain::scheduler::schedule_rule::RuleKind;
use domain::scheduler::EffectiveSchedule;

use crate::errors::AppError;
use crate::scheduler::ports::{EffectiveIntervalStore, ResourceRepository, ScheduleRuleRepository};

pub const MATERIALIZATION_DAYS: i64 = 90;

/// Computes the effective schedule for a resource by intersecting it with all ancestors.
/// Resources without availability rules are transparent (don't restrict).
pub async fn compute_effective_schedule(
    resource_id: Uuid,
    from: NaiveDate,
    until: NaiveDate,
    resource_repo: &Arc<dyn ResourceRepository>,
    rule_repo: &Arc<dyn ScheduleRuleRepository>,
) -> Result<EffectiveSchedule, AppError> {
    let ancestors = resource_repo.find_ancestors(resource_id).await?;

    // Chain from root to leaf so we intersect top-down
    let chain: Vec<Uuid> = ancestors
        .iter()
        .rev()
        .map(|r| r.id)
        .chain(std::iter::once(resource_id))
        .collect();

    let mut combined: Option<EffectiveSchedule> = None;
    for id in &chain {
        let rules = rule_repo.find_by_resource(*id).await?;
        let has_availability = rules.iter().any(|r| r.kind == RuleKind::Availability);
        if has_availability {
            let schedule = EffectiveSchedule::compute(&rules, from, until);
            combined = Some(match combined {
                None => schedule,
                Some(existing) => existing.intersect_with(&schedule),
            });
        }
        // No availability rules → resource is transparent, doesn't restrict
    }

    Ok(combined.unwrap_or_default())
}

/// Rematerializes effective_intervals for `resource_id` and all direct+indirect
/// children that have `inherits_parent_schedule = true`.
pub async fn rematerialize(
    root_id: Uuid,
    resource_repo: &Arc<dyn ResourceRepository>,
    rule_repo: &Arc<dyn ScheduleRuleRepository>,
    interval_store: &Arc<dyn EffectiveIntervalStore>,
) -> Result<(), AppError> {
    let from = chrono::Utc::now().date_naive();
    let until = from + chrono::Duration::days(MATERIALIZATION_DAYS);

    let mut queue = vec![root_id];
    while let Some(resource_id) = queue.pop() {
        rematerialize_single(resource_id, from, until, resource_repo, rule_repo, interval_store)
            .await?;
        let children = resource_repo.find_children(resource_id).await?;
        for child in children.into_iter().filter(|c| c.inherits_parent_schedule) {
            queue.push(child.id);
        }
    }
    Ok(())
}

async fn rematerialize_single(
    resource_id: Uuid,
    from: NaiveDate,
    until: NaiveDate,
    resource_repo: &Arc<dyn ResourceRepository>,
    rule_repo: &Arc<dyn ScheduleRuleRepository>,
    interval_store: &Arc<dyn EffectiveIntervalStore>,
) -> Result<(), AppError> {
    let resource = resource_repo
        .find_by_id(resource_id)
        .await?
        .ok_or_else(|| {
            AppError::Domain(domain::errors::DomainError::NotFound(format!(
                "Resource {resource_id}"
            )))
        })?;

    let schedule =
        compute_effective_schedule(resource_id, from, until, resource_repo, rule_repo).await?;

    let mut tuples: Vec<(NaiveDate, domain::scheduler::TimeInterval, i32)> = Vec::new();
    for (date, intervals) in &schedule.intervals {
        for iv in intervals {
            tuples.push((*date, iv.clone(), resource.max_concurrent_events));
        }
    }

    interval_store
        .replace_for_resource(resource_id, from, until, &tuples)
        .await
}
