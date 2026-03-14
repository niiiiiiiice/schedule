use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use domain::errors::DomainError;
use domain::events::DomainEvent;
use domain::scheduler::errors::SchedulerError;
use domain::scheduler::events::SchedulerEvent;

use crate::errors::AppError;
use crate::ports::EventDispatcher;
use crate::scheduler::ports::{BookingRepository, EffectiveIntervalStore, ResourceRepository, ScheduleRuleRepository};
use crate::scheduler::schedule_utils::{rematerialize, MATERIALIZATION_DAYS};

pub struct DeleteScheduleRuleCommand {
    pub rule_id: Uuid,
    pub resource_id: Uuid,
}

pub struct DeleteScheduleRuleHandler {
    resource_repo: Arc<dyn ResourceRepository>,
    rule_repo: Arc<dyn ScheduleRuleRepository>,
    interval_store: Arc<dyn EffectiveIntervalStore>,
    booking_repo: Arc<dyn BookingRepository>,
    dispatcher: Arc<dyn EventDispatcher>,
}

impl DeleteScheduleRuleHandler {
    pub fn new(
        resource_repo: Arc<dyn ResourceRepository>,
        rule_repo: Arc<dyn ScheduleRuleRepository>,
        interval_store: Arc<dyn EffectiveIntervalStore>,
        booking_repo: Arc<dyn BookingRepository>,
        dispatcher: Arc<dyn EventDispatcher>,
    ) -> Self {
        Self { resource_repo, rule_repo, interval_store, booking_repo, dispatcher }
    }

    pub async fn handle(&self, cmd: DeleteScheduleRuleCommand) -> Result<(), AppError> {
        if self.rule_repo.find_by_id(cmd.rule_id).await?.is_none() {
            return Err(AppError::Domain(DomainError::NotFound(format!(
                "Schedule rule {}", cmd.rule_id
            ))));
        }

        // Check bookings won't be stranded after deletion
        self.check_conflicts_after_deletion(&cmd).await?;

        self.rule_repo.delete(cmd.rule_id).await?;

        rematerialize(
            cmd.resource_id,
            &self.resource_repo,
            &self.rule_repo,
            &self.interval_store,
        )
        .await?;

        self.dispatcher
            .dispatch(&[DomainEvent::Scheduler(SchedulerEvent::ScheduleRuleDeleted {
                resource_id: cmd.resource_id,
                rule_id: cmd.rule_id,
                deleted_at: Utc::now(),
            })])
            .await?;

        Ok(())
    }

    async fn check_conflicts_after_deletion(
        &self,
        cmd: &DeleteScheduleRuleCommand,
    ) -> Result<(), AppError> {
        let from = chrono::Utc::now().date_naive();
        let until = from + chrono::Duration::days(MATERIALIZATION_DAYS);

        // Rules after deletion
        let mut rules = self.rule_repo.find_by_resource(cmd.resource_id).await?;
        rules.retain(|r| r.id != cmd.rule_id);

        let new_schedule = domain::scheduler::EffectiveSchedule::compute(&rules, from, until);

        let period_start = from.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let period_end = until.and_hms_opt(23, 59, 59).unwrap().and_utc();
        let bookings = self
            .booking_repo
            .find_confirmed_in_period(cmd.resource_id, period_start, period_end)
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
