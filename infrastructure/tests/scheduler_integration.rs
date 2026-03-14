/// Integration tests for the scheduler system.
///
/// Require a running PostgreSQL instance. Set TEST_DATABASE_URL or use the default.
/// Run with: cargo test -p infrastructure -- --ignored --test-threads=1
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{NaiveDate, NaiveTime, Weekday};

use application::dispatcher::InProcessEventDispatcher;
use application::errors::AppError;
use application::ports::DomainEventHandler;
use application::scheduler::commands::cancel_booking::{CancelBookingCommand, CancelBookingHandler};
use application::scheduler::commands::create_booking::{CreateBookingCommand, CreateBookingHandler};
use application::scheduler::commands::create_resource::{CreateResourceCommand, CreateResourceHandler};
use application::scheduler::commands::create_schedule_rule::{
    CreateScheduleRuleCommand, CreateScheduleRuleHandler,
};
use application::scheduler::commands::update_schedule_rule::{
    UpdateScheduleRuleCommand, UpdateScheduleRuleHandler,
};
use application::scheduler::ports::{BookingRepository, EffectiveIntervalStore, ResourceRepository, ScheduleRuleRepository};
use application::scheduler::queries::get_resource_schedule::{
    GetResourceScheduleHandler, GetResourceScheduleQuery,
};
use domain::errors::DomainError;
use domain::events::DomainEvent;
use domain::scheduler::errors::SchedulerError;
use domain::scheduler::schedule_rule::{RecurrenceParams, RuleKind};
use infrastructure::postgres::{
    self, PgBookingRepository, PgEffectiveIntervalStore, PgResourceRepository,
    PgScheduleRuleRepository,
};
use sqlx::PgPool;

// ─── Test infrastructure ─────────────────────────────────────────────────────

async fn make_pool() -> PgPool {
    let url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1@localhost:5432/scheduler_test".into());
    let pool = postgres::create_pool(&url).await.expect("connect to test DB");
    postgres::run_migrations(&pool).await.expect("run migrations");
    pool
}

async fn truncate(pool: &PgPool) {
    sqlx::query(
        "TRUNCATE booking_resources, bookings, effective_intervals, schedule_rules, resources",
    )
    .execute(pool)
    .await
    .expect("truncate tables");
}

/// No-op event handler — swallows all domain events in tests.
struct Noop;

#[async_trait]
impl DomainEventHandler for Noop {
    async fn handle(&self, _event: &DomainEvent) -> Result<(), AppError> {
        Ok(())
    }
}

fn dispatcher() -> Arc<InProcessEventDispatcher> {
    Arc::new(InProcessEventDispatcher::new(vec![Arc::new(Noop)]))
}

/// Known Monday for deterministic test dates.
/// 2026-03-16 is a Monday.
fn test_monday() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 3, 16).unwrap()
}

fn t(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

struct Repos {
    resource_repo: Arc<dyn ResourceRepository>,
    rule_repo: Arc<dyn ScheduleRuleRepository>,
    interval_store: Arc<dyn EffectiveIntervalStore>,
    booking_repo: Arc<dyn BookingRepository>,
}

fn make_repos(pool: &PgPool) -> Repos {
    Repos {
        resource_repo: Arc::new(PgResourceRepository::new(pool.clone())),
        rule_repo: Arc::new(PgScheduleRuleRepository::new(pool.clone())),
        interval_store: Arc::new(PgEffectiveIntervalStore::new(pool.clone())),
        booking_repo: Arc::new(PgBookingRepository::new(pool.clone())),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

/// Full booking lifecycle:
/// 1. Create resource hierarchy (office → room → doctor)
/// 2. Add Mon-Fri 09:00-18:00 availability rule + 12:00-13:00 break rule to office
/// 3. Verify doctor's effective schedule excludes the lunch break
/// 4. Book a slot within available hours → succeeds
/// 5. Book the same slot again (capacity=1) → CapacityExceeded
/// 6. Try to update the rule so the booked slot falls outside → RuleConflictsWithBookings
/// 7. Cancel the booking → retry the rule update → succeeds
#[tokio::test]
#[ignore]
async fn test_full_booking_lifecycle() {
    let pool = make_pool().await;
    truncate(&pool).await;
    let r = make_repos(&pool);
    let d = dispatcher();

    // ── 1. Resource hierarchy ────────────────────────────────────────────────
    let create_resource =
        CreateResourceHandler::new(r.resource_repo.clone(), d.clone());

    let office = create_resource
        .handle(CreateResourceCommand {
            name: "Main Office".into(),
            parent_id: None,
            max_concurrent_events: 10,
            inherits_parent_schedule: false,
        })
        .await
        .expect("create office");

    let room = create_resource
        .handle(CreateResourceCommand {
            name: "Room 1".into(),
            parent_id: Some(office.id),
            max_concurrent_events: 2,
            inherits_parent_schedule: true,
        })
        .await
        .expect("create room");

    let doctor = create_resource
        .handle(CreateResourceCommand {
            name: "Dr. Smith".into(),
            parent_id: Some(room.id),
            max_concurrent_events: 1,
            inherits_parent_schedule: true,
        })
        .await
        .expect("create doctor");

    // ── 2. Availability rules on office ─────────────────────────────────────
    let create_rule = CreateScheduleRuleHandler::new(
        r.resource_repo.clone(),
        r.rule_repo.clone(),
        r.interval_store.clone(),
        r.booking_repo.clone(),
        d.clone(),
    );

    let weekdays = vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri];

    let availability_rule = create_rule
        .handle(CreateScheduleRuleCommand {
            resource_id: office.id,
            kind: RuleKind::Availability,
            recurrence: RecurrenceParams::Daily {
                days_of_week: weekdays.clone(),
                start_time: t(9, 0),
                end_time: t(18, 0),
            },
            priority: 0,
            effective_from: None,
            effective_until: None,
        })
        .await
        .expect("create availability rule");

    create_rule
        .handle(CreateScheduleRuleCommand {
            resource_id: office.id,
            kind: RuleKind::Break,
            recurrence: RecurrenceParams::Daily {
                days_of_week: weekdays.clone(),
                start_time: t(12, 0),
                end_time: t(13, 0),
            },
            priority: 10,
            effective_from: None,
            effective_until: None,
        })
        .await
        .expect("create break rule");

    // ── 3. Verify effective schedule of doctor ───────────────────────────────
    let get_schedule = GetResourceScheduleHandler::new(
        r.resource_repo.clone(),
        r.rule_repo.clone(),
        r.interval_store.clone(),
    );
    let monday = test_monday();

    let schedule = get_schedule
        .handle(GetResourceScheduleQuery {
            resource_id: doctor.id,
            from: monday,
            until: monday,
        })
        .await
        .expect("get doctor schedule");

    // Should have two intervals: 09:00-12:00 and 13:00-18:00
    assert_eq!(schedule.len(), 2, "expected 2 intervals on Monday (break excluded)");
    let morning = &schedule[0];
    let afternoon = &schedule[1];
    assert_eq!(morning.start, t(9, 0));
    assert_eq!(morning.end, t(12, 0));
    assert_eq!(afternoon.start, t(13, 0));
    assert_eq!(afternoon.end, t(18, 0));

    // ── 4. Book a slot within available hours ────────────────────────────────
    let create_booking = CreateBookingHandler::new(
        r.resource_repo.clone(),
        r.rule_repo.clone(),
        r.interval_store.clone(),
        r.booking_repo.clone(),
        d.clone(),
    );

    let slot_start = monday.and_time(t(10, 0)).and_utc();
    let slot_end = monday.and_time(t(11, 0)).and_utc();

    let booking = create_booking
        .handle(CreateBookingCommand {
            resource_ids: vec![doctor.id],
            start_at: slot_start,
            end_at: slot_end,
            metadata: None,
        })
        .await
        .expect("create booking in available slot");

    assert_eq!(booking.status, "confirmed");

    // ── 5. Same slot again (capacity = 1) → CapacityExceeded ────────────────
    let err = create_booking
        .handle(CreateBookingCommand {
            resource_ids: vec![doctor.id],
            start_at: slot_start,
            end_at: slot_end,
            metadata: None,
        })
        .await
        .expect_err("should fail: capacity exceeded");

    assert!(
        matches!(
            err,
            AppError::Domain(DomainError::Scheduler(SchedulerError::CapacityExceeded))
        ),
        "expected CapacityExceeded, got: {err:?}"
    );

    // ── 6. Update rule so 10:00-11:00 is no longer available ─────────────────
    //    Shift availability start to 11:00 → booking at 10:00 conflicts
    let update_rule = UpdateScheduleRuleHandler::new(
        r.resource_repo.clone(),
        r.rule_repo.clone(),
        r.interval_store.clone(),
        r.booking_repo.clone(),
        d.clone(),
    );

    let conflict_err = update_rule
        .handle(UpdateScheduleRuleCommand {
            rule_id: availability_rule.id,
            resource_id: office.id,
            kind: RuleKind::Availability,
            recurrence: RecurrenceParams::Daily {
                days_of_week: weekdays.clone(),
                start_time: t(11, 0), // was 09:00 — now 10:00 slot is outside
                end_time: t(18, 0),
            },
            priority: 0,
            effective_from: None,
            effective_until: None,
        })
        .await
        .expect_err("should fail: conflicts with existing booking");

    assert!(
        matches!(
            conflict_err,
            AppError::Domain(DomainError::Scheduler(
                SchedulerError::RuleConflictsWithBookings(1)
            ))
        ),
        "expected RuleConflictsWithBookings(1), got: {conflict_err:?}"
    );

    // ── 7. Cancel booking, retry rule update → succeeds ──────────────────────
    let cancel = CancelBookingHandler::new(r.booking_repo.clone(), d.clone());
    cancel
        .handle(CancelBookingCommand { booking_id: booking.id })
        .await
        .expect("cancel booking");

    update_rule
        .handle(UpdateScheduleRuleCommand {
            rule_id: availability_rule.id,
            resource_id: office.id,
            kind: RuleKind::Availability,
            recurrence: RecurrenceParams::Daily {
                days_of_week: weekdays,
                start_time: t(11, 0),
                end_time: t(18, 0),
            },
            priority: 0,
            effective_from: None,
            effective_until: None,
        })
        .await
        .expect("update rule after cancellation");

    // Verify the updated schedule no longer has 09:00-11:00
    let updated_schedule = get_schedule
        .handle(GetResourceScheduleQuery {
            resource_id: doctor.id,
            from: monday,
            until: monday,
        })
        .await
        .expect("get updated doctor schedule");

    assert_eq!(updated_schedule.len(), 2, "still 2 intervals after break");
    assert_eq!(updated_schedule[0].start, t(11, 0), "morning now starts at 11:00");
    assert_eq!(updated_schedule[0].end, t(12, 0));
    assert_eq!(updated_schedule[1].start, t(13, 0));
}

/// Multi-resource booking: both resources must be available.
/// If one resource has no schedule (or booking conflicts), the whole booking fails.
#[tokio::test]
#[ignore]
async fn test_multi_resource_booking() {
    let pool = make_pool().await;
    truncate(&pool).await;
    let r = make_repos(&pool);
    let d = dispatcher();

    let create_resource = CreateResourceHandler::new(r.resource_repo.clone(), d.clone());
    let create_rule = CreateScheduleRuleHandler::new(
        r.resource_repo.clone(),
        r.rule_repo.clone(),
        r.interval_store.clone(),
        r.booking_repo.clone(),
        d.clone(),
    );
    let create_booking = CreateBookingHandler::new(
        r.resource_repo.clone(),
        r.rule_repo.clone(),
        r.interval_store.clone(),
        r.booking_repo.clone(),
        d.clone(),
    );

    // Two independent resources, each with Mon-Fri availability
    let res_a = create_resource
        .handle(CreateResourceCommand {
            name: "Resource A".into(),
            parent_id: None,
            max_concurrent_events: 1,
            inherits_parent_schedule: false,
        })
        .await
        .expect("create A");

    let res_b = create_resource
        .handle(CreateResourceCommand {
            name: "Resource B".into(),
            parent_id: None,
            max_concurrent_events: 1,
            inherits_parent_schedule: false,
        })
        .await
        .expect("create B");

    let weekdays = vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri];

    for &rid in &[res_a.id, res_b.id] {
        create_rule
            .handle(CreateScheduleRuleCommand {
                resource_id: rid,
                kind: RuleKind::Availability,
                recurrence: RecurrenceParams::Daily {
                    days_of_week: weekdays.clone(),
                    start_time: t(9, 0),
                    end_time: t(17, 0),
                },
                priority: 0,
                effective_from: None,
                effective_until: None,
            })
            .await
            .expect("create rule");
    }

    let monday = test_monday();
    let start = monday.and_time(t(10, 0)).and_utc();
    let end = monday.and_time(t(11, 0)).and_utc();

    // Book both resources together
    let booking = create_booking
        .handle(CreateBookingCommand {
            resource_ids: vec![res_a.id, res_b.id],
            start_at: start,
            end_at: end,
            metadata: None,
        })
        .await
        .expect("multi-resource booking");

    assert_eq!(booking.resource_ids.len(), 2);

    // Second booking for the same resources and time → CapacityExceeded
    let err = create_booking
        .handle(CreateBookingCommand {
            resource_ids: vec![res_a.id, res_b.id],
            start_at: start,
            end_at: end,
            metadata: None,
        })
        .await
        .expect_err("should fail: capacity exceeded");

    assert!(
        matches!(
            err,
            AppError::Domain(DomainError::Scheduler(SchedulerError::CapacityExceeded))
        ),
        "expected CapacityExceeded, got {err:?}"
    );
}

/// Booking outside schedule hours → ScheduleConflict.
#[tokio::test]
#[ignore]
async fn test_booking_outside_schedule_rejected() {
    let pool = make_pool().await;
    truncate(&pool).await;
    let r = make_repos(&pool);
    let d = dispatcher();

    let create_resource = CreateResourceHandler::new(r.resource_repo.clone(), d.clone());
    let create_rule = CreateScheduleRuleHandler::new(
        r.resource_repo.clone(),
        r.rule_repo.clone(),
        r.interval_store.clone(),
        r.booking_repo.clone(),
        d.clone(),
    );
    let create_booking = CreateBookingHandler::new(
        r.resource_repo.clone(),
        r.rule_repo.clone(),
        r.interval_store.clone(),
        r.booking_repo.clone(),
        d.clone(),
    );

    let res = create_resource
        .handle(CreateResourceCommand {
            name: "Limited Resource".into(),
            parent_id: None,
            max_concurrent_events: 5,
            inherits_parent_schedule: false,
        })
        .await
        .expect("create resource");

    create_rule
        .handle(CreateScheduleRuleCommand {
            resource_id: res.id,
            kind: RuleKind::Availability,
            recurrence: RecurrenceParams::Daily {
                days_of_week: vec![Weekday::Mon],
                start_time: t(9, 0),
                end_time: t(12, 0),
            },
            priority: 0,
            effective_from: None,
            effective_until: None,
        })
        .await
        .expect("create rule");

    let monday = test_monday();

    // Booking spans the lunch hour — partially outside schedule → rejected
    let err = create_booking
        .handle(CreateBookingCommand {
            resource_ids: vec![res.id],
            start_at: monday.and_time(t(11, 0)).and_utc(),
            end_at: monday.and_time(t(13, 0)).and_utc(), // extends past 12:00 cutoff
            metadata: None,
        })
        .await
        .expect_err("booking outside schedule should fail");

    assert!(
        matches!(
            err,
            AppError::Domain(DomainError::Scheduler(SchedulerError::ScheduleConflict))
        ),
        "expected ScheduleConflict, got {err:?}"
    );

    // Booking fully within schedule → succeeds
    create_booking
        .handle(CreateBookingCommand {
            resource_ids: vec![res.id],
            start_at: monday.and_time(t(10, 0)).and_utc(),
            end_at: monday.and_time(t(11, 30)).and_utc(),
            metadata: None,
        })
        .await
        .expect("valid booking within schedule");
}
