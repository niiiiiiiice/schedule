use std::collections::HashMap;

use chrono::{NaiveDate, NaiveDateTime};

use super::schedule_rule::{RuleKind, ScheduleRule};
use super::time_interval::TimeInterval;

/// Вычисленное эффективное расписание для одного ресурса на диапазон дат.
#[derive(Debug, Clone, Default)]
pub struct EffectiveSchedule {
    /// Ключ — дата, значение — отсортированные непересекающиеся доступные интервалы.
    pub intervals: HashMap<NaiveDate, Vec<TimeInterval>>,
}

impl EffectiveSchedule {
    /// Вычисляет расписание из набора правил ресурса для диапазона [from, until].
    /// Алгоритм: union(availability) − union(breaks) для каждого дня.
    pub fn compute(rules: &[ScheduleRule], from: NaiveDate, until: NaiveDate) -> Self {
        let mut schedule = EffectiveSchedule::default();

        let mut date = from;
        loop {
            let day_intervals = Self::compute_for_date(rules, date);
            if !day_intervals.is_empty() {
                schedule.intervals.insert(date, day_intervals);
            }
            match date.succ_opt() {
                Some(next) if next <= until => date = next,
                _ => break,
            }
        }

        schedule
    }

    fn compute_for_date(rules: &[ScheduleRule], date: NaiveDate) -> Vec<TimeInterval> {
        let mut availability: Vec<TimeInterval> = Vec::new();
        for rule in rules.iter().filter(|r| r.kind == RuleKind::Availability) {
            let pairs = rule.generate_intervals(date, date);
            availability.extend(pairs.into_iter().map(|(_, i)| i));
        }
        let availability = TimeInterval::union(availability);

        let mut breaks: Vec<TimeInterval> = Vec::new();
        for rule in rules.iter().filter(|r| r.kind == RuleKind::Break) {
            let pairs = rule.generate_intervals(date, date);
            breaks.extend(pairs.into_iter().map(|(_, i)| i));
        }
        let breaks = TimeInterval::union(breaks);

        TimeInterval::subtract(&availability, &breaks)
    }

    /// Пересечение двух расписаний (расписание дочернего ∩ расписание родителя).
    pub fn intersect_with(&self, other: &EffectiveSchedule) -> EffectiveSchedule {
        let mut result = EffectiveSchedule::default();
        for (date, intervals_a) in &self.intervals {
            if let Some(intervals_b) = other.intervals.get(date) {
                let intersected = TimeInterval::intersect(intervals_a, intervals_b);
                if !intersected.is_empty() {
                    result.intervals.insert(*date, intersected);
                }
            }
        }
        result
    }

    /// Проверяет, полностью ли покрывает расписание интервал [start, end).
    /// Кросс-суточные бронирования не поддерживаются (возвращает false).
    pub fn contains_booking(&self, start: NaiveDateTime, end: NaiveDateTime) -> bool {
        if start.date() != end.date() {
            return false;
        }
        let date = start.date();
        let Some(day_intervals) = self.intervals.get(&date) else {
            return false;
        };
        let Some(booking) = TimeInterval::new(start.time(), end.time()) else {
            return false;
        };
        day_intervals.iter().any(|i| i.contains_interval(&booking))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::schedule_rule::{RecurrenceParams, RuleKind};
    use chrono::{NaiveDate, NaiveTime, Weekday};
    use uuid::Uuid;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn time(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn make_rule(kind: RuleKind, recurrence: RecurrenceParams) -> ScheduleRule {
        ScheduleRule::new(Uuid::new_v4(), kind, recurrence, 0, None, None)
    }

    #[test]
    fn compute_weekday_availability_with_break() {
        let monday = date(2026, 3, 16); // Monday
        let rules = vec![
            make_rule(
                RuleKind::Availability,
                RecurrenceParams::Daily {
                    days_of_week: vec![
                        Weekday::Mon,
                        Weekday::Tue,
                        Weekday::Wed,
                        Weekday::Thu,
                        Weekday::Fri,
                    ],
                    start_time: time(9, 0),
                    end_time: time(18, 0),
                },
            ),
            make_rule(
                RuleKind::Break,
                RecurrenceParams::Daily {
                    days_of_week: vec![],
                    start_time: time(12, 0),
                    end_time: time(13, 0),
                },
            ),
        ];
        let schedule = EffectiveSchedule::compute(&rules, monday, monday);
        let day = schedule.intervals.get(&monday).expect("should have intervals");
        assert_eq!(day.len(), 2);
        assert_eq!(day[0].start, time(9, 0));
        assert_eq!(day[0].end, time(12, 0));
        assert_eq!(day[1].start, time(13, 0));
        assert_eq!(day[1].end, time(18, 0));
    }

    #[test]
    fn intersect_narrows_schedule() {
        let day = date(2026, 3, 16);

        let wide = EffectiveSchedule::compute(
            &[make_rule(
                RuleKind::Availability,
                RecurrenceParams::Daily {
                    days_of_week: vec![],
                    start_time: time(8, 0),
                    end_time: time(20, 0),
                },
            )],
            day,
            day,
        );

        let narrow = EffectiveSchedule::compute(
            &[make_rule(
                RuleKind::Availability,
                RecurrenceParams::Daily {
                    days_of_week: vec![],
                    start_time: time(10, 0),
                    end_time: time(17, 0),
                },
            )],
            day,
            day,
        );

        let result = wide.intersect_with(&narrow);
        let intervals = result.intervals.get(&day).unwrap();
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].start, time(10, 0));
        assert_eq!(intervals[0].end, time(17, 0));
    }
}
