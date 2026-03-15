use std::sync::Arc;

use chrono::{NaiveDate, NaiveTime, TimeDelta};
use uuid::Uuid;

use domain::errors::DomainError;
use domain::scheduler::booking::Booking;
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

    pub async fn handle(
        &self,
        query: GetAvailableSlotsQuery,
    ) -> Result<Vec<AvailableSlot>, AppError> {
        let resource = self
            .resource_repo
            .find_by_id(query.resource_id)
            .await?
            .ok_or_else(|| {
                AppError::Domain(DomainError::NotFound(format!(
                    "Resource {}",
                    query.resource_id
                )))
            })?;

        // Load effective intervals (materialized or on-the-fly)
        let rows = self
            .interval_store
            .get_for_resource_and_range(query.resource_id, query.from, query.until)
            .await?;

        let intervals_by_date: Vec<(NaiveDate, Vec<TimeInterval>, i32)> = if !rows.is_empty() {
            let mut map: std::collections::HashMap<NaiveDate, Vec<TimeInterval>> =
                std::collections::HashMap::new();
            for row in &rows {
                map.entry(row.date).or_default().push(row.to_time_interval());
            }
            let capacity = rows
                .first()
                .map(|r| r.available_capacity)
                .unwrap_or(resource.max_concurrent_events);
            let mut pairs: Vec<_> =
                map.into_iter().map(|(d, ivs)| (d, ivs, capacity)).collect();
            pairs.sort_by_key(|(d, _, _)| *d);
            pairs
        } else {
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

        let period_start = query.from.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let period_end = query.until.and_hms_opt(23, 59, 59).unwrap().and_utc();
        let bookings = self
            .booking_repo
            .find_confirmed_in_period(query.resource_id, period_start, period_end)
            .await?;

        let mut result = Vec::new();

        for (date, day_intervals, max_cap) in intervals_by_date {
            for iv in &day_intervals {
                match query.duration_minutes {
                    None => {
                        // Split the interval by booking boundaries so that partially-booked
                        // intervals still show available sub-intervals.
                        let sub = split_by_bookings(date, iv.start, iv.end, &bookings, max_cap);
                        for (start, end, remaining) in sub {
                            result.push(AvailableSlot {
                                date,
                                start,
                                end,
                                remaining_capacity: remaining,
                            });
                        }
                    }
                    Some(mins) => {
                        // Slice into fixed-duration slots; compute overlap per slot.
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

                            let slot_start_utc = date.and_time(slot_start).and_utc();
                            let slot_end_utc = date.and_time(slot_end).and_utc();
                            let overlap = bookings
                                .iter()
                                .filter(|b| {
                                    b.start_at < slot_end_utc && b.end_at > slot_start_utc
                                })
                                .count() as i32;
                            let remaining = max_cap - overlap;
                            if remaining > 0 {
                                result.push(AvailableSlot {
                                    date,
                                    start: slot_start,
                                    end: slot_end,
                                    remaining_capacity: remaining,
                                });
                            }
                            slot_start = slot_end;
                        }
                    }
                }
            }
        }
        Ok(result)
    }
}

/// Splits the interval [iv_start, iv_end] on a given date by the start/end times of
/// overlapping bookings. Returns sub-intervals where remaining capacity > 0,
/// merging adjacent segments that have the same remaining capacity.
fn split_by_bookings(
    date: NaiveDate,
    iv_start: NaiveTime,
    iv_end: NaiveTime,
    bookings: &[Booking],
    max_cap: i32,
) -> Vec<(NaiveTime, NaiveTime, i32)> {
    let iv_start_utc = date.and_time(iv_start).and_utc();
    let iv_end_utc = date.and_time(iv_end).and_utc();

    // Collect breakpoints: interval boundaries + booking start/end times that fall
    // strictly inside the interval.
    let mut breakpoints: Vec<NaiveTime> = vec![iv_start, iv_end];
    for b in bookings {
        if b.start_at >= iv_end_utc || b.end_at <= iv_start_utc {
            continue; // doesn't overlap this interval
        }
        let b_start = b.start_at.naive_utc().time();
        let b_end = b.end_at.naive_utc().time();
        if b_start > iv_start && b_start < iv_end {
            breakpoints.push(b_start);
        }
        if b_end > iv_start && b_end < iv_end {
            breakpoints.push(b_end);
        }
    }
    breakpoints.sort();
    breakpoints.dedup();

    let mut result: Vec<(NaiveTime, NaiveTime, i32)> = Vec::new();

    for window in breakpoints.windows(2) {
        let (seg_start, seg_end) = (window[0], window[1]);
        let seg_start_utc = date.and_time(seg_start).and_utc();
        let seg_end_utc = date.and_time(seg_end).and_utc();

        let overlap = bookings
            .iter()
            .filter(|b| b.start_at < seg_end_utc && b.end_at > seg_start_utc)
            .count() as i32;
        let remaining = max_cap - overlap;

        if remaining <= 0 {
            continue;
        }

        // Merge with previous segment if same remaining capacity.
        if let Some(last) = result.last_mut() {
            if last.1 == seg_start && last.2 == remaining {
                last.1 = seg_end;
                continue;
            }
        }
        result.push((seg_start, seg_end, remaining));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};
    use domain::scheduler::booking::{Booking, BookingStatus};
    use uuid::Uuid;

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn make_booking(date: NaiveDate, start_h: u32, start_m: u32, end_h: u32, end_m: u32) -> Booking {
        let start = date.and_time(t(start_h, start_m)).and_utc();
        let end = date.and_time(t(end_h, end_m)).and_utc();
        Booking::from_raw(Uuid::new_v4(), vec![Uuid::new_v4()], start, end, BookingStatus::Confirmed, None, Utc::now(), Utc::now())
    }

    /// Regression: a booking in the middle of an interval must NOT hide
    /// the portions before and after the booking.
    #[test]
    fn booking_in_middle_leaves_surrounding_free() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 17).unwrap();
        let booking = make_booking(date, 14, 20, 14, 40);
        let bookings = vec![booking];

        let slots = split_by_bookings(date, t(9, 0), t(17, 0), &bookings, 1);

        assert_eq!(slots.len(), 2, "expected 2 free sub-intervals, not 0");
        assert_eq!(slots[0], (t(9, 0), t(14, 20), 1));
        assert_eq!(slots[1], (t(14, 40), t(17, 0), 1));
    }

    /// With capacity=2 and one booking, the whole interval still appears as a
    /// single segment with remaining=1.
    #[test]
    fn partial_booking_with_capacity_2() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 17).unwrap();
        let booking = make_booking(date, 10, 0, 11, 0);
        let bookings = vec![booking];

        let slots = split_by_bookings(date, t(9, 0), t(17, 0), &bookings, 2);

        // Before booking: remaining=2, during: remaining=1, after: remaining=2
        // Segments before and after are NOT merged because during (remaining=1) breaks the run
        assert_eq!(slots.len(), 3);
        assert_eq!(slots[0], (t(9, 0), t(10, 0), 2));
        assert_eq!(slots[1], (t(10, 0), t(11, 0), 1));
        assert_eq!(slots[2], (t(11, 0), t(17, 0), 2));
    }

    /// No bookings → single slot with full capacity.
    #[test]
    fn no_bookings_returns_full_interval() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 17).unwrap();
        let slots = split_by_bookings(date, t(9, 0), t(17, 0), &[], 1);
        assert_eq!(slots, vec![(t(9, 0), t(17, 0), 1)]);
    }

    /// Booking fills the entire interval → no slots returned.
    #[test]
    fn fully_booked_interval_returns_empty() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 17).unwrap();
        let booking = make_booking(date, 9, 0, 17, 0);
        let slots = split_by_bookings(date, t(9, 0), t(17, 0), &[booking], 1);
        assert!(slots.is_empty(), "fully booked interval should return no slots");
    }
}
