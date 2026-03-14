use std::sync::Arc;

use chrono::{NaiveDate, TimeDelta};
use uuid::Uuid;

use domain::errors::DomainError;
use domain::scheduler::TimeInterval;

use crate::errors::AppError;
use crate::scheduler::dto::AvailableSlot;
use crate::scheduler::ports::{
    BookingRepository, EffectiveIntervalStore, ResourceRepository, ScheduleRuleRepository,
};
use crate::scheduler::schedule_utils::compute_effective_schedule;

pub struct GetAvailableSlotsQuery {
    pub resource_id: Uuid,
    pub from: NaiveDate,
    pub until: NaiveDate,
    /// If set, slice intervals into slots of this duration.
    pub duration_minutes: Option<u32>,
}

pub struct GetAvailableSlotsHandler {
    resource_repo: Arc<dyn ResourceRepository>,
    rule_repo: Arc<dyn ScheduleRuleRepository>,
    interval_store: Arc<dyn EffectiveIntervalStore>,
    booking_repo: Arc<dyn BookingRepository>,
}

impl GetAvailableSlotsHandler {
    pub fn new(
        resource_repo: Arc<dyn ResourceRepository>,
        rule_repo: Arc<dyn ScheduleRuleRepository>,
        interval_store: Arc<dyn EffectiveIntervalStore>,
        booking_repo: Arc<dyn BookingRepository>,
    ) -> Self {
        Self { resource_repo, rule_repo, interval_store, booking_repo }
    }

    pub async fn handle(&self, query: GetAvailableSlotsQuery) -> Result<Vec<AvailableSlot>, AppError> {
        let resource = self
            .resource_repo
            .find_by_id(query.resource_id)
            .await?
            .ok_or_else(|| {
                AppError::Domain(DomainError::NotFound(format!("Resource {}", query.resource_id)))
            })?;

        // Load effective intervals (materialized or on-the-fly)
        let rows = self
            .interval_store
            .get_for_resource_and_range(query.resource_id, query.from, query.until)
            .await?;

        let intervals_by_date: Vec<(NaiveDate, Vec<TimeInterval>, i32)> = if !rows.is_empty() {
            // Group by date
            let mut map: std::collections::HashMap<NaiveDate, Vec<TimeInterval>> =
                std::collections::HashMap::new();
            for row in &rows {
                map.entry(row.date)
                    .or_default()
                    .push(row.to_time_interval());
            }
            let capacity = rows.first().map(|r| r.available_capacity).unwrap_or(resource.max_concurrent_events);
            let mut pairs: Vec<_> = map.into_iter().map(|(d, ivs)| (d, ivs, capacity)).collect();
            pairs.sort_by_key(|(d, _, _)| *d);
            pairs
        } else {
            // On-the-fly
            let schedule = compute_effective_schedule(
                query.resource_id,
                query.from,
                query.until,
                &self.resource_repo,
                &self.rule_repo,
            )
            .await?;
            let mut pairs: Vec<_> = schedule
                .intervals
                .into_iter()
                .map(|(d, ivs)| (d, ivs, resource.max_concurrent_events))
                .collect();
            pairs.sort_by_key(|(d, _, _)| *d);
            pairs
        };

        // For each interval, compute remaining capacity
        let period_start = query.from.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let period_end = query.until.and_hms_opt(23, 59, 59).unwrap().and_utc();
        let bookings = self
            .booking_repo
            .find_confirmed_in_period(query.resource_id, period_start, period_end)
            .await?;

        let mut result = Vec::new();
        for (date, day_intervals, max_cap) in intervals_by_date {
            for iv in &day_intervals {
                let iv_start = date.and_time(iv.start).and_utc();
                let iv_end = date.and_time(iv.end).and_utc();

                let overlap_count = bookings
                    .iter()
                    .filter(|b| b.start_at < iv_end && b.end_at > iv_start)
                    .count() as i32;

                let remaining = max_cap - overlap_count;
                if remaining <= 0 {
                    continue;
                }

                match query.duration_minutes {
                    None => result.push(AvailableSlot {
                        date,
                        start: iv.start,
                        end: iv.end,
                        remaining_capacity: remaining,
                    }),
                    Some(mins) => {
                        // Slice into fixed-size slots
                        let slot_duration = TimeDelta::minutes(mins as i64);
                        let mut slot_start = iv.start;
                        loop {
                            let slot_end_dt =
                                date.and_time(slot_start).and_utc() + slot_duration;
                            if slot_end_dt.date_naive() != date {
                                break;
                            }
                            let slot_end = slot_end_dt.time();
                            if slot_end > iv.end {
                                break;
                            }
                            result.push(AvailableSlot {
                                date,
                                start: slot_start,
                                end: slot_end,
                                remaining_capacity: remaining,
                            });
                            slot_start = slot_end;
                        }
                    }
                }
            }
        }
        Ok(result)
    }
}
