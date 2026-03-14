use chrono::{Datelike, NaiveDate, NaiveTime, Weekday};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::time_interval::TimeInterval;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RuleKind {
    Availability,
    Break,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "type")]
pub enum RecurrenceParams {
    /// Single occurrence on a specific date
    Once {
        date: NaiveDate,
        start_time: NaiveTime,
        end_time: NaiveTime,
    },
    /// Repeats on specified weekdays (empty = every day)
    Daily {
        /// Weekday names: Mon, Tue, Wed, Thu, Fri, Sat, Sun
        #[schema(value_type = Vec<String>)]
        days_of_week: Vec<Weekday>,
        start_time: NaiveTime,
        end_time: NaiveTime,
    },
    /// Repeats every week_interval weeks on specified weekdays
    Weekly {
        week_interval: u32,
        anchor_date: NaiveDate,
        /// Weekday names: Mon, Tue, Wed, Thu, Fri, Sat, Sun
        #[schema(value_type = Vec<String>)]
        days_of_week: Vec<Weekday>,
        start_time: NaiveTime,
        end_time: NaiveTime,
    },
    /// Repeats every every_n_days days starting from anchor_date
    Custom {
        every_n_days: u32,
        anchor_date: NaiveDate,
        start_time: NaiveTime,
        end_time: NaiveTime,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScheduleRule {
    pub id: Uuid,
    pub resource_id: Uuid,
    pub kind: RuleKind,
    pub recurrence: RecurrenceParams,
    pub priority: i32,
    /// Правило не применяется до этой даты
    pub effective_from: Option<NaiveDate>,
    /// Правило не применяется после этой даты
    pub effective_until: Option<NaiveDate>,
}

impl ScheduleRule {
    pub fn new(
        resource_id: Uuid,
        kind: RuleKind,
        recurrence: RecurrenceParams,
        priority: i32,
        effective_from: Option<NaiveDate>,
        effective_until: Option<NaiveDate>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            resource_id,
            kind,
            recurrence,
            priority,
            effective_from,
            effective_until,
        }
    }

    /// Генерирует (дата, интервал) пары для диапазона дат [from, until] включительно.
    pub fn generate_intervals(
        &self,
        from: NaiveDate,
        until: NaiveDate,
    ) -> Vec<(NaiveDate, TimeInterval)> {
        let from = self.effective_from.map(|ef| from.max(ef)).unwrap_or(from);
        let until = self.effective_until.map(|eu| until.min(eu)).unwrap_or(until);

        if from > until {
            return vec![];
        }

        let mut result = Vec::new();
        let mut date = from;
        loop {
            if let Some(interval) = self.interval_for_date(date) {
                result.push((date, interval));
            }
            match date.succ_opt() {
                Some(next) if next <= until => date = next,
                _ => break,
            }
        }
        result
    }

    fn interval_for_date(&self, date: NaiveDate) -> Option<TimeInterval> {
        match &self.recurrence {
            RecurrenceParams::Once {
                date: rule_date,
                start_time,
                end_time,
            } => {
                if date == *rule_date {
                    TimeInterval::new(*start_time, *end_time)
                } else {
                    None
                }
            }
            RecurrenceParams::Daily {
                days_of_week,
                start_time,
                end_time,
            } => {
                if days_of_week.is_empty() || days_of_week.contains(&date.weekday()) {
                    TimeInterval::new(*start_time, *end_time)
                } else {
                    None
                }
            }
            RecurrenceParams::Weekly {
                week_interval,
                anchor_date,
                days_of_week,
                start_time,
                end_time,
            } => {
                let days_since_anchor = (date - *anchor_date).num_days();
                if days_since_anchor < 0 {
                    return None;
                }
                let week_num = days_since_anchor / 7;
                if week_num % (*week_interval as i64) == 0
                    && days_of_week.contains(&date.weekday())
                {
                    TimeInterval::new(*start_time, *end_time)
                } else {
                    None
                }
            }
            RecurrenceParams::Custom {
                every_n_days,
                anchor_date,
                start_time,
                end_time,
            } => {
                let days_since_anchor = (date - *anchor_date).num_days();
                if days_since_anchor >= 0
                    && days_since_anchor % (*every_n_days as i64) == 0
                {
                    TimeInterval::new(*start_time, *end_time)
                } else {
                    None
                }
            }
        }
    }
}
