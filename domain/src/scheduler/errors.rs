#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Schedule conflict: booking does not fit into available intervals")]
    ScheduleConflict,

    #[error("Capacity exceeded: resource has no available slots")]
    CapacityExceeded,

    #[error("Rule change would conflict with {0} existing booking(s)")]
    RuleConflictsWithBookings(usize),

    #[error("Invalid time range: start must be before end")]
    InvalidTimeRange,
}
