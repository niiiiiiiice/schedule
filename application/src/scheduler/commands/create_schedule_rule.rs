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

pub struct CreateScheduleRuleCommand {
    pub resource_id: Uuid,
    pub kind: RuleKind,
    pub recurrence: RecurrenceParams,
    pub priority: i32,
    pub effective_from: Option<NaiveDate>,
    pub effective_until: Option<NaiveDate>,
}

pub struct CreateScheduleRuleHandler {
    resource_repo: Arc<dyn ResourceRepository>,
    rule_repo: Arc<dyn ScheduleRuleRepository>,
    interval_store: Arc<dyn EffectiveIntervalStore>,
    booking_repo: Arc<dyn BookingRepository>,
    dispatcher: Arc<dyn EventDispatcher>,
}

impl CreateScheduleRuleHandler {
    pub fn new(
        resource_repo: Arc<dyn ResourceRepository>,
        rule_repo: Arc<dyn ScheduleRuleRepository>,
        interval_store: Arc<dyn EffectiveIntervalStore>,
        booking_repo: Arc<dyn BookingRepository>,
        dispatcher: Arc<dyn EventDispatcher>,
    ) -> Self {
        Self { resource_repo, rule_repo, interval_store, booking_repo, dispatcher }
    }

    pub async fn handle(&self, cmd: CreateScheduleRuleCommand) -> Result<ScheduleRule, AppError> {
        if self.resource_repo.find_by_id(cmd.resource_id).await?.is_none() {
            return Err(AppError::Domain(DomainError::NotFound(format!(
                "Resource {}", cmd.resource_id
            ))));
        }

        let rule = ScheduleRule::new(
            cmd.resource_id,
            cmd.kind,
            cmd.recurrence,
            cmd.priority,
            cmd.effective_from,
            cmd.effective_until,
        );

        // Check new rule doesn't conflict with existing bookings
        self.check_conflicts_after_adding(&rule).await?;

        self.rule_repo.save(&rule).await?;

        rematerialize(
            cmd.resource_id,
            &self.resource_repo,
            &self.rule_repo,
            &self.interval_store,
        )
        .await?;

        self.dispatcher
            .dispatch(&[DomainEvent::Scheduler(SchedulerEvent::ScheduleRuleCreated {
                resource_id: cmd.resource_id,
                rule_id: rule.id,
                created_at: Utc::now(),
            })])
            .await?;

        Ok(rule)
    }

    async fn check_conflicts_after_adding(&self, new_rule: &ScheduleRule) -> Result<(), AppError> {
        let from = chrono::Utc::now().date_naive();
        let until = from + chrono::Duration::days(MATERIALIZATION_DAYS);

        // Load existing rules + the new one
        let mut rules = self.rule_repo.find_by_resource(new_rule.resource_id).await?;
        rules.push(new_rule.clone());

        let new_schedule = domain::scheduler::EffectiveSchedule::compute(&rules, from, until);

        let period_start = from.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let period_end = until.and_hms_opt(23, 59, 59).unwrap().and_utc();
        let bookings = self
            .booking_repo
            .find_confirmed_in_period(new_rule.resource_id, period_start, period_end)
            .await?;

        let conflicts: Vec<_> = bookings
            .iter()
            .filter(|b| {
                !new_schedule
                    .contains_booking(b.start_at.naive_utc(), b.end_at.naive_utc())
            })
            .collect();

        if !conflicts.is_empty() {
            return Err(AppError::from(SchedulerError::RuleConflictsWithBookings(conflicts.len())));
        }
        Ok(())
    }
}
