use std::sync::Arc;

use chrono::NaiveDate;
use uuid::Uuid;

use domain::errors::DomainError;

use crate::errors::AppError;
use crate::scheduler::dto::ScheduleIntervalDto;
use crate::scheduler::ports::{EffectiveIntervalStore, ResourceRepository, ScheduleRuleRepository};
use crate::scheduler::schedule_utils::compute_effective_schedule;

pub struct GetResourceScheduleQuery {
    pub resource_id: Uuid,
    pub from: NaiveDate,
    pub until: NaiveDate,
}

pub struct GetResourceScheduleHandler {
    resource_repo: Arc<dyn ResourceRepository>,
    rule_repo: Arc<dyn ScheduleRuleRepository>,
    interval_store: Arc<dyn EffectiveIntervalStore>,
}

impl GetResourceScheduleHandler {
    pub fn new(
        resource_repo: Arc<dyn ResourceRepository>,
        rule_repo: Arc<dyn ScheduleRuleRepository>,
        interval_store: Arc<dyn EffectiveIntervalStore>,
    ) -> Self {
        Self { resource_repo, rule_repo, interval_store }
    }

    pub async fn handle(
        &self,
        query: GetResourceScheduleQuery,
    ) -> Result<Vec<ScheduleIntervalDto>, AppError> {
        if self.resource_repo.find_by_id(query.resource_id).await?.is_none() {
            return Err(AppError::Domain(DomainError::NotFound(format!(
                "Resource {}", query.resource_id
            ))));
        }

        // Try materialized intervals first
        let rows = self
            .interval_store
            .get_for_resource_and_range(query.resource_id, query.from, query.until)
            .await?;

        if !rows.is_empty() {
            return Ok(rows
                .into_iter()
                .map(|r| ScheduleIntervalDto {
                    date: r.date,
                    start: r.start_time,
                    end: r.end_time,
                    available_capacity: r.available_capacity,
                })
                .collect());
        }

        // Fall back to on-the-fly computation
        let resource = self
            .resource_repo
            .find_by_id(query.resource_id)
            .await?
            .unwrap(); // already checked above

        let schedule = compute_effective_schedule(
            query.resource_id,
            query.from,
            query.until,
            &self.resource_repo,
            &self.rule_repo,
        )
        .await?;

        let mut result = Vec::new();
        let mut dates: Vec<_> = schedule.intervals.keys().cloned().collect();
        dates.sort();
        for date in dates {
            for iv in &schedule.intervals[&date] {
                result.push(ScheduleIntervalDto {
                    date,
                    start: iv.start,
                    end: iv.end,
                    available_capacity: resource.max_concurrent_events,
                });
            }
        }
        Ok(result)
    }
}
