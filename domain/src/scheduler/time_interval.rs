use chrono::NaiveTime;
use serde::{Deserialize, Serialize};

/// Временной интервал в рамках одного дня [start, end).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TimeInterval {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

impl TimeInterval {
    /// Создаёт интервал, возвращает None если start >= end.
    pub fn new(start: NaiveTime, end: NaiveTime) -> Option<Self> {
        if start < end {
            Some(Self { start, end })
        } else {
            None
        }
    }

    pub fn overlaps_with(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    pub fn contains_interval(&self, other: &Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// Слияние перекрывающихся интервалов. Принимает произвольный порядок.
    pub fn union(mut intervals: Vec<Self>) -> Vec<Self> {
        if intervals.is_empty() {
            return vec![];
        }
        intervals.sort_by_key(|i| i.start);
        let mut result: Vec<Self> = Vec::new();
        for interval in intervals {
            if let Some(last) = result.last_mut() {
                if interval.start <= last.end {
                    if interval.end > last.end {
                        last.end = interval.end;
                    }
                } else {
                    result.push(interval);
                }
            } else {
                result.push(interval);
            }
        }
        result
    }

    /// Пересечение двух отсортированных списков непересекающихся интервалов.
    pub fn intersect(a: &[Self], b: &[Self]) -> Vec<Self> {
        let mut result = Vec::new();
        let mut i = 0;
        let mut j = 0;
        while i < a.len() && j < b.len() {
            let start = a[i].start.max(b[j].start);
            let end = a[i].end.min(b[j].end);
            if start < end {
                result.push(Self { start, end });
            }
            if a[i].end < b[j].end {
                i += 1;
            } else {
                j += 1;
            }
        }
        result
    }

    /// Вычитание: base минус все cuts. base должен быть отсортирован.
    pub fn subtract(base: &[Self], cuts: &[Self]) -> Vec<Self> {
        let mut result: Vec<Self> = base.to_vec();
        for cut in cuts {
            let mut next = Vec::new();
            for interval in &result {
                if cut.end <= interval.start || cut.start >= interval.end {
                    next.push(interval.clone());
                    continue;
                }
                if interval.start < cut.start {
                    next.push(Self {
                        start: interval.start,
                        end: cut.start,
                    });
                }
                if cut.end < interval.end {
                    next.push(Self {
                        start: cut.end,
                        end: interval.end,
                    });
                }
            }
            result = next;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn iv(sh: u32, sm: u32, eh: u32, em: u32) -> TimeInterval {
        TimeInterval::new(t(sh, sm), t(eh, em)).unwrap()
    }

    #[test]
    fn union_merges_overlapping() {
        let intervals = vec![iv(9, 0, 12, 0), iv(11, 0, 14, 0), iv(16, 0, 18, 0)];
        let result = TimeInterval::union(intervals);
        assert_eq!(result, vec![iv(9, 0, 14, 0), iv(16, 0, 18, 0)]);
    }

    #[test]
    fn union_merges_adjacent() {
        let intervals = vec![iv(9, 0, 12, 0), iv(12, 0, 14, 0)];
        let result = TimeInterval::union(intervals);
        assert_eq!(result, vec![iv(9, 0, 14, 0)]);
    }

    #[test]
    fn intersect_basic() {
        let a = vec![iv(9, 0, 18, 0)];
        let b = vec![iv(10, 0, 15, 0)];
        assert_eq!(TimeInterval::intersect(&a, &b), vec![iv(10, 0, 15, 0)]);
    }

    #[test]
    fn intersect_no_overlap() {
        let a = vec![iv(9, 0, 12, 0)];
        let b = vec![iv(13, 0, 18, 0)];
        assert!(TimeInterval::intersect(&a, &b).is_empty());
    }

    #[test]
    fn subtract_middle() {
        let base = vec![iv(9, 0, 18, 0)];
        let cuts = vec![iv(12, 0, 13, 0)];
        assert_eq!(
            TimeInterval::subtract(&base, &cuts),
            vec![iv(9, 0, 12, 0), iv(13, 0, 18, 0)]
        );
    }

    #[test]
    fn subtract_no_overlap() {
        let base = vec![iv(9, 0, 12, 0)];
        let cuts = vec![iv(14, 0, 16, 0)];
        assert_eq!(TimeInterval::subtract(&base, &cuts), vec![iv(9, 0, 12, 0)]);
    }

    #[test]
    fn subtract_multiple_cuts() {
        let base = vec![iv(9, 0, 18, 0)];
        let cuts = vec![iv(10, 0, 11, 0), iv(12, 0, 13, 0)];
        assert_eq!(
            TimeInterval::subtract(&base, &cuts),
            vec![iv(9, 0, 10, 0), iv(11, 0, 12, 0), iv(13, 0, 18, 0)]
        );
    }

    #[test]
    fn contains_interval() {
        let outer = iv(9, 0, 18, 0);
        assert!(outer.contains_interval(&iv(10, 0, 17, 0)));
        assert!(!outer.contains_interval(&iv(8, 0, 17, 0)));
    }
}
