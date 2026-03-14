use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use uuid::Uuid;

use domain::errors::DomainError;
use domain::events::DomainEvent;
use domain::scheduler::errors::SchedulerError;
use domain::scheduler::events::SchedulerEvent;
use domain::scheduler::schedule_rule::{RecurrenceParams, RuleKind};
use domain::scheduler::ScheduleRule;

use crate::errors::AppError;
use crate::ports::EventDispatcher;
use crate::scheduler::ports::{BookingRepository, EffectiveIntervalStore, ResourceRepository, ScheduleRuleRepository};
use crate::scheduler::schedule_utils::{rematerialize, MATERIALIZATION_DAYS};

pub struct UpdateScheduleRuleCommand {
    pub rule_id: Uuid,
    pub resource_id: Uuid,
    pub kind: RuleKind,
    pub recurrence: RecurrenceParams,
    pub priority: i32,
    pub effective_from: Option<NaiveDate>,
    pub effective_until: Option<NaiveDate>,
}

pub struct UpdateScheduleRuleHandler {
    resource_repo: Arc<dyn ResourceRepository>,
    rule_repo: Arc<dyn ScheduleRuleRepository>,
    interval_store: Arc<dyn EffectiveIntervalStore>,
    booking_repo: Arc<dyn BookingRepository>,
    dispatcher: Arc<dyn EventDispatcher>,
}

impl UpdateScheduleRuleHandler {
    pub fn new(
        resource_repo: Arc<dyn ResourceRepository>,
        rule_repo: Arc<dyn ScheduleRuleRepository>,
        interval_store: Arc<dyn EffectiveIntervalStore>,
        booking_repo: Arc<dyn BookingRepository>,
        dispatcher: Arc<dyn EventDispatcher>,
    ) -> Self {
        Self { resource_repo, rule_repo, interval_store, booking_repo, dispatcher }
    }

    pub async fn handle(&self, cmd: UpdateScheduleRuleCommand) -> Result<ScheduleRule, AppError> {
        if self.rule_repo.find_by_id(cmd.rule_id).await?.is_none() {
            return Err(AppError::Domain(DomainError::NotFound(format!(
                "Schedule rule {}", cmd.rule_id
            ))));
        }

        let updated_rule = ScheduleRule {
            id: cmd.rule_id,
            resource_id: cmd.resource_id,
            kind: cmd.kind,
            recurrence: cmd.recurrence,
            priority: cmd.priority,
            effective_from: cmd.effective_from,
            effective_until: cmd.effective_until,
        };

        self.check_conflicts(&updated_rule).await?;

        self.rule_repo.update(&updated_rule).await?;

        rematerialize(
            cmd.resource_id,
            &self.resource_repo,
            &self.rule_repo,
            &self.interval_store,
        )
        .await?;

        self.dispatcher
            .dispatch(&[DomainEvent::Scheduler(SchedulerEvent::ScheduleRuleUpdated {
                resource_id: cmd.resource_id,
                rule_id: cmd.rule_id,
                updated_at: Utc::now(),
            })])
            .await?;

        Ok(updated_rule)
    }

    async fn check_conflicts(&self, updated_rule: &ScheduleRule) -> Result<(), AppError> {
        let from = chrono::Utc::now().date_naive();
        let until = from + chrono::Duration::days(MATERIALIZATION_DAYS);

        // Build rule set with the updated rule replacing the old one
        let mut rules = self.rule_repo.find_by_resource(updated_rule.resource_id).await?;
        rules.retain(|r| r.id != updated_rule.id);
        rules.push(updated_rule.clone());

        let new_schedule = domain::scheduler::EffectiveSchedule::compute(&rules, from, until);

        let period_start = from.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let period_end = until.and_hms_opt(23, 59, 59).unwrap().and_utc();
        let bookings = self
            .booking_repo
            .find_confirmed_in_period(updated_rule.resource_id, period_start, period_end)
            .await?;

        let conflicts = bookings
            .iter()
            .filter(|b| !new_schedule.contains_booking(b.start_at.naive_utc(), b.end_at.naive_utc()))
            .count();

        if conflicts > 0 {
            return Err(AppError::from(SchedulerError::RuleConflictsWithBookings(conflicts)));
        }
        Ok(())
    }
}
