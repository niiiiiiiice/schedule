use std::sync::Arc;

use chrono::DateTime;
use chrono::Utc;
use uuid::Uuid;

use domain::errors::DomainError;
use domain::events::DomainEvent;
use domain::scheduler::errors::SchedulerError;
use domain::scheduler::events::SchedulerEvent;
use domain::scheduler::Booking;
use domain::scheduler::EffectiveSchedule;
use domain::scheduler::TimeInterval;

use crate::errors::AppError;
use crate::ports::EventDispatcher;
use crate::scheduler::dto::BookingDto;
use crate::scheduler::ports::{
    BookingRepository, EffectiveIntervalStore, ResourceRepository, ScheduleRuleRepository,
};
use crate::scheduler::schedule_utils::compute_effective_schedule;

pub struct CreateBookingCommand {
    pub resource_ids: Vec<Uuid>,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

pub struct CreateBookingHandler {
    resource_repo: Arc<dyn ResourceRepository>,
    rule_repo: Arc<dyn ScheduleRuleRepository>,
    interval_store: Arc<dyn EffectiveIntervalStore>,
    booking_repo: Arc<dyn BookingRepository>,
    dispatcher: Arc<dyn EventDispatcher>,
}

impl CreateBookingHandler {
    pub fn new(
        resource_repo: Arc<dyn ResourceRepository>,
        rule_repo: Arc<dyn ScheduleRuleRepository>,
        interval_store: Arc<dyn EffectiveIntervalStore>,
        booking_repo: Arc<dyn BookingRepository>,
        dispatcher: Arc<dyn EventDispatcher>,
    ) -> Self {
        Self { resource_repo, rule_repo, interval_store, booking_repo, dispatcher }
    }

    pub async fn handle(&self, cmd: CreateBookingCommand) -> Result<BookingDto, AppError> {
        let booking =
            Booking::new(cmd.resource_ids.clone(), cmd.start_at, cmd.end_at, cmd.metadata)?;

        let booking_date = cmd.start_at.date_naive();
        let start_naive = cmd.start_at.naive_utc();
        let end_naive = cmd.end_at.naive_utc();

        let mut resource_max_concurrent: Vec<(Uuid, i32)> = Vec::new();

        for &resource_id in &cmd.resource_ids {
            let resource = self
                .resource_repo
                .find_by_id(resource_id)
                .await?
                .ok_or_else(|| {
                    AppError::Domain(DomainError::NotFound(format!("Resource {resource_id}")))
                })?;

            // Try materialized intervals first, fall back to on-the-fly
            let schedule = self
                .load_schedule(resource_id, booking_date)
                .await?;

            if !schedule.contains_booking(start_naive, end_naive) {
                return Err(AppError::from(SchedulerError::ScheduleConflict));
            }

            resource_max_concurrent.push((resource_id, resource.max_concurrent_events));
        }

        // Atomic insert with advisory locks + capacity check inside booking_repo
        self.booking_repo
            .create_atomic(&booking, &resource_max_concurrent)
            .await?;

        self.dispatcher
            .dispatch(&[DomainEvent::Scheduler(SchedulerEvent::BookingCreated {
                booking_id: booking.id,
                resource_ids: booking.resource_ids.clone(),
                start_at: booking.start_at,
                end_at: booking.end_at,
                created_at: Utc::now(),
            })])
            .await?;

        Ok(BookingDto::from(booking))
    }

    async fn load_schedule(
        &self,
        resource_id: Uuid,
        date: chrono::NaiveDate,
    ) -> Result<EffectiveSchedule, AppError> {
        let rows = self
            .interval_store
            .get_for_resource_and_range(resource_id, date, date)
            .await?;

        if !rows.is_empty() {
            let mut schedule = EffectiveSchedule::default();
            let intervals: Vec<TimeInterval> = rows.iter().map(|r| r.to_time_interval()).collect();
            schedule.intervals.insert(date, intervals);
            return Ok(schedule);
        }

        // Fall back: compute from rules
        compute_effective_schedule(resource_id, date, date, &self.resource_repo, &self.rule_repo)
            .await
    }
}
