# Scheduler System Implementation Plan

## Context

Implementing a production-ready resource scheduler on top of the existing Rust CQRS API (todo template at `C:\Projects\rust-cqrs-api\rust-cqrs-api`). The system manages resources (e.g. doctors, offices, rooms) with hierarchical availability rules and multi-resource event bookings. Key challenges: concurrent booking safety, rule-change validation against existing events, horizontal scalability (multiple Kubernetes pods), and materialized schedule intervals for read performance.

Architecture agreed in design discussion:
- Stateless services coordinating via PostgreSQL advisory locks
- Hybrid materialization: `effective_intervals` table (90-day horizon) + on-the-fly for beyond
- Rule changes validated against existing events before saving
- Multi-resource bookings lock all resource IDs in ascending order (deadlock prevention)

## Cleanup: Remove Todo Functionality

Before implementing the scheduler, remove all Todo/Task-related code — it's no longer needed and the scheduler replaces it entirely.

**Files to delete:**
- `domain/src/entities.rs` — Task aggregate
- `domain/src/value_objects.rs` — TaskTitle, TaskDescription, TaskStatus
- `domain/src/events/task_events.rs` + `domain/src/events.rs`
- `application/src/commands/create_task.rs`
- `application/src/commands/update_task_status.rs`
- `application/src/commands/delete_task.rs`
- `application/src/queries/get_task.rs`
- `application/src/queries/list_tasks.rs`
- `application/src/dto.rs` — TaskListItem, TaskDetail
- `infrastructure/src/postgres/write_repo.rs` — PgTaskWriteRepository
- `infrastructure/src/postgres/read_repo.rs` — PgTaskReadRepository
- `infrastructure/src/event_handlers/task_handlers.rs` — TaskCacheInvalidationHandler
- `api/src/handlers/tasks.rs` — task HTTP handlers

**Files to clean (remove task references, keep structure):**
- `domain/src/lib.rs` — remove task module exports
- `application/src/lib.rs` — remove task commands/queries exports
- `application/src/ports.rs` — remove `TaskWriteRepository`, `TaskReadRepository`, `Cache` traits (or repurpose)
- `application/src/errors.rs` — keep error types, remove task-specific error variants if any
- `application/src/dispatcher.rs` — keep dispatcher, remove task handler registration
- `infrastructure/src/postgres/mod.rs` — remove task migrations, keep DB setup
- `infrastructure/src/event_handlers/mod.rs` — remove task cache invalidation handler
- `infrastructure/src/lib.rs` — remove task repo exports
- `api/src/state.rs` — remove task handler fields
- `api/src/main.rs` — remove task wiring, keep server setup
- `api/src/openapi.rs` — remove task schemas
- `api/src/handlers/mod.rs` — remove tasks handler module

---

## Files to Create / Modify

### Phase 1: Domain Layer (`domain/src/`)

**New files:**
- `domain/src/scheduler/mod.rs` — module root
- `domain/src/scheduler/resource.rs` — `Resource` aggregate
- `domain/src/scheduler/schedule_rule.rs` — `ScheduleRule` entity + rule type enums
- `domain/src/scheduler/booking.rs` — `Booking` aggregate (multi-resource event)
- `domain/src/scheduler/time_interval.rs` — `TimeInterval` value object + interval algebra
- `domain/src/scheduler/effective_schedule.rs` — `EffectiveSchedule` type + calculator trait
- `domain/src/scheduler/errors.rs` — `SchedulerDomainError`
- `domain/src/scheduler/events.rs` — `SchedulerEvent` domain events

**Modify:**
- `domain/src/lib.rs` — add `pub mod scheduler;`

### Phase 2: Application Layer (`application/src/`)

**New files:**
- `application/src/scheduler/mod.rs`
- `application/src/scheduler/ports.rs` — trait definitions
- `application/src/scheduler/dto.rs` — DTOs for queries
- `application/src/scheduler/commands/create_resource.rs`
- `application/src/scheduler/commands/create_schedule_rule.rs`
- `application/src/scheduler/commands/update_schedule_rule.rs`
- `application/src/scheduler/commands/delete_schedule_rule.rs`
- `application/src/scheduler/commands/create_booking.rs`
- `application/src/scheduler/commands/cancel_booking.rs`
- `application/src/scheduler/queries/get_available_slots.rs`
- `application/src/scheduler/queries/get_resource_schedule.rs`
- `application/src/scheduler/queries/list_resources.rs`
- `application/src/scheduler/schedule_calculator.rs` — `ScheduleCalculatorImpl` (pure logic)

**Modify:**
- `application/src/lib.rs` — add `pub mod scheduler;`
- `application/src/errors.rs` — add `Scheduler(SchedulerDomainError)` variant

### Phase 3: Infrastructure Layer (`infrastructure/src/`)

**New files:**
- `infrastructure/src/postgres/scheduler_migrations.rs` — SQL schema
- `infrastructure/src/postgres/resource_repo.rs` — `PgResourceRepository`
- `infrastructure/src/postgres/schedule_rule_repo.rs` — `PgScheduleRuleRepository`
- `infrastructure/src/postgres/booking_repo.rs` — `PgBookingRepository`
- `infrastructure/src/postgres/effective_interval_store.rs` — `PgEffectiveIntervalStore`

**Modify:**
- `infrastructure/src/postgres/mod.rs` — add `pub mod scheduler_migrations; pub use ...;` call `run_scheduler_migrations()`
- `infrastructure/src/lib.rs` — re-export new repos

### Phase 4: API Layer (`api/src/`)

**New files:**
- `api/src/handlers/scheduler.rs` — HTTP handlers
- `api/src/handlers/resources.rs`

**Modify:**
- `api/src/state.rs` — add scheduler handler fields
- `api/src/main.rs` — wire repos, add routes
- `api/src/openapi.rs` — add scheduler schemas

---

## SQL Schema (Migrations)

```sql
-- resources
CREATE TABLE IF NOT EXISTS resources (
    id                       UUID PRIMARY KEY,
    name                     VARCHAR(200) NOT NULL,
    parent_id                UUID REFERENCES resources(id) ON DELETE RESTRICT,
    max_concurrent_events    INT NOT NULL DEFAULT 1 CHECK (max_concurrent_events >= 1),
    inherits_parent_schedule BOOLEAN NOT NULL DEFAULT false,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_resources_parent_id ON resources(parent_id);

-- schedule_rules
-- rule_kind: 'availability' | 'break'
-- recurrence_type: 'once' | 'daily' | 'weekly' | 'custom'
-- parameters JSONB examples:
--   weekly:   {"days_of_week": [1,2,3,4,5], "start_time": "09:00", "end_time": "18:00"}
--   once:     {"date": "2026-03-20", "start_time": "12:00", "end_time": "13:00"}
--   custom:   {"every_n_days": 14, "anchor_date": "2026-01-05", "start_time": "08:00", "end_time": "16:00"}
CREATE TABLE IF NOT EXISTS schedule_rules (
    id               UUID PRIMARY KEY,
    resource_id      UUID NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
    rule_kind        VARCHAR(20) NOT NULL CHECK (rule_kind IN ('availability','break')),
    recurrence_type  VARCHAR(20) NOT NULL CHECK (recurrence_type IN ('once','daily','weekly','custom')),
    priority         INT NOT NULL DEFAULT 0,
    effective_from   DATE,
    effective_until  DATE,
    parameters       JSONB NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_schedule_rules_resource ON schedule_rules(resource_id, rule_kind);

-- effective_intervals (materialized schedule, 90-day horizon)
CREATE TABLE IF NOT EXISTS effective_intervals (
    id                 BIGSERIAL PRIMARY KEY,
    resource_id        UUID NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
    date               DATE NOT NULL,
    start_time         TIME NOT NULL,
    end_time           TIME NOT NULL,
    available_capacity INT NOT NULL,
    computed_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_effective_intervals_resource_date_time
    ON effective_intervals(resource_id, date, start_time);

-- bookings (the event / appointment)
CREATE TABLE IF NOT EXISTS bookings (
    id         UUID PRIMARY KEY,
    start_at   TIMESTAMPTZ NOT NULL,
    end_at     TIMESTAMPTZ NOT NULL,
    status     VARCHAR(20) NOT NULL DEFAULT 'confirmed' CHECK (status IN ('confirmed','cancelled')),
    metadata   JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT bookings_time_check CHECK (end_at > start_at)
);

-- booking_resources (multi-resource many-to-many, denormalized with times for overlap index)
CREATE TABLE IF NOT EXISTS booking_resources (
    booking_id  UUID NOT NULL REFERENCES bookings(id) ON DELETE CASCADE,
    resource_id UUID NOT NULL REFERENCES resources(id) ON DELETE RESTRICT,
    start_at    TIMESTAMPTZ NOT NULL,
    end_at      TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (booking_id, resource_id)
);
CREATE INDEX IF NOT EXISTS idx_booking_resources_resource_time
    ON booking_resources(resource_id, start_at, end_at);
```

---

## Key Domain Types

```rust
// domain/src/scheduler/time_interval.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeInterval {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

impl TimeInterval {
    pub fn union(intervals: Vec<Self>) -> Vec<Self> { /* merge overlapping */ }
    pub fn intersect(a: &[Self], b: &[Self]) -> Vec<Self> { /* two-pointer */ }
    pub fn subtract(base: &[Self], cuts: &[Self]) -> Vec<Self> { /* remove cuts from base */ }
    pub fn overlaps_with(&self, other: &Self) -> bool { ... }
    pub fn contains_interval(&self, other: &Self) -> bool { ... }
}

// domain/src/scheduler/schedule_rule.rs
#[derive(Clone, Debug)]
pub enum RuleKind { Availability, Break }

#[derive(Clone, Debug)]
pub enum RecurrenceType {
    Once { date: NaiveDate, start_time: NaiveTime, end_time: NaiveTime },
    Daily { days_of_week: Vec<Weekday>, start_time: NaiveTime, end_time: NaiveTime },
    Weekly { week_interval: u32, days_of_week: Vec<Weekday>, start_time: NaiveTime, end_time: NaiveTime },
    Custom { every_n_days: u32, anchor_date: NaiveDate, start_time: NaiveTime, end_time: NaiveTime },
}

pub struct ScheduleRule {
    pub id: Uuid,
    pub resource_id: Uuid,
    pub kind: RuleKind,
    pub recurrence: RecurrenceType,
    pub priority: i32,
    pub effective_from: Option<NaiveDate>,
    pub effective_until: Option<NaiveDate>,
}

impl ScheduleRule {
    pub fn generate_intervals(&self, date_range: (NaiveDate, NaiveDate)) -> Vec<(NaiveDate, TimeInterval)> { ... }
}

// domain/src/scheduler/effective_schedule.rs
pub struct EffectiveSchedule {
    pub intervals: HashMap<NaiveDate, Vec<TimeInterval>>,
    pub capacity: i32,
}

impl EffectiveSchedule {
    pub fn compute(rules: &[ScheduleRule], date_range: (NaiveDate, NaiveDate)) -> Self { ... }
    pub fn intersect_with(&self, other: &Self) -> Self { ... }
    pub fn contains_booking(&self, start: NaiveDateTime, end: NaiveDateTime) -> bool { ... }
}
```

---

## Application Ports (Traits)

```rust
// application/src/scheduler/ports.rs

#[async_trait]
pub trait ResourceRepository: Send + Sync {
    async fn save(&self, resource: &Resource) -> Result<(), AppError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Resource>, AppError>;
    async fn find_children(&self, parent_id: Uuid) -> Result<Vec<Resource>, AppError>;
    async fn find_ancestors(&self, id: Uuid) -> Result<Vec<Resource>, AppError>; // walks to root
    async fn list_all(&self) -> Result<Vec<Resource>, AppError>;
}

#[async_trait]
pub trait ScheduleRuleRepository: Send + Sync {
    async fn save(&self, rule: &ScheduleRule) -> Result<(), AppError>;
    async fn update(&self, rule: &ScheduleRule) -> Result<(), AppError>;
    async fn delete(&self, id: Uuid) -> Result<(), AppError>;
    async fn find_by_resource(&self, resource_id: Uuid) -> Result<Vec<ScheduleRule>, AppError>;
    async fn find_by_resource_and_period(
        &self, resource_id: Uuid, from: NaiveDate, until: NaiveDate
    ) -> Result<Vec<ScheduleRule>, AppError>;
}

#[async_trait]
pub trait BookingRepository: Send + Sync {
    async fn save(&self, booking: &Booking) -> Result<(), AppError>;
    async fn cancel(&self, id: Uuid) -> Result<(), AppError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Booking>, AppError>;
    async fn find_overlapping_for_resource(
        &self, resource_id: Uuid, start: DateTime<Utc>, end: DateTime<Utc>
    ) -> Result<Vec<Booking>, AppError>;
    async fn find_by_resource_in_period(
        &self, resource_id: Uuid, from: DateTime<Utc>, until: DateTime<Utc>
    ) -> Result<Vec<Booking>, AppError>;
}

#[async_trait]
pub trait EffectiveIntervalStore: Send + Sync {
    async fn get_for_resource_date(
        &self, resource_id: Uuid, date: NaiveDate
    ) -> Result<Vec<EffectiveIntervalRow>, AppError>;
    async fn replace_for_resource(
        &self, resource_id: Uuid, from: NaiveDate, until: NaiveDate,
        intervals: &[(NaiveDate, TimeInterval, i32)]
    ) -> Result<(), AppError>;
    async fn delete_for_resource(&self, resource_id: Uuid) -> Result<(), AppError>;
}
```

---

## Command Handler Logic

### `CreateBookingHandler`
1. Sort `resource_ids` ascending (deadlock prevention)
2. Begin DB transaction
3. `pg_advisory_xact_lock(hash(resource_id))` for each resource (in sorted order)
4. For each resource: load effective schedule (from `effective_intervals` table OR compute on-the-fly)
5. Verify `[start_at, end_at]` falls within available intervals
6. Count existing confirmed bookings in that window per resource → check `< max_concurrent_events`
7. Insert into `bookings` + `booking_resources`
8. Commit

### `UpdateScheduleRuleHandler`
1. Acquire advisory lock on resource_id
2. Compute new effective schedule with updated rule
3. Find all confirmed bookings in the affected period for this resource AND all inheriting children
4. Check every booking is still within new schedule — if any conflict → return `ConflictingBookings` error with list
5. Save updated rule
6. Recompute and replace `effective_intervals` for resource + all `inherits_parent_schedule=true` children (recursively)
7. Commit

### `GetAvailableSlotsHandler` (query)
1. Load `effective_intervals` for resource_id and date range
2. Load confirmed bookings for resource_id in date range
3. For each interval: subtract time occupied by bookings (respecting capacity)
4. Optionally slice into slots of `desired_duration` minutes
5. Return list of `AvailableSlot { date, start, end, remaining_capacity }`

---

## API Endpoints

```
POST   /resources                           → CreateResource
GET    /resources                           → ListResources
GET    /resources/{id}                      → GetResource
GET    /resources/{id}/schedule             → GetResourceSchedule (effective schedule for date range)
GET    /resources/{id}/availability         → GetAvailableSlots (query: from, to, duration_minutes)

POST   /resources/{id}/schedule-rules       → CreateScheduleRule
PUT    /resources/{id}/schedule-rules/{rid} → UpdateScheduleRule (validates against existing bookings)
DELETE /resources/{id}/schedule-rules/{rid} → DeleteScheduleRule (validates against existing bookings)

POST   /bookings                            → CreateBooking (body: resource_ids[], start_at, end_at, metadata)
GET    /bookings/{id}                       → GetBooking
POST   /bookings/{id}/cancel                → CancelBooking
```

---

## Implementation Order

1. **Cleanup** — удалить весь Todo/Task код (см. раздел выше)
2. **Domain** — `time_interval.rs` (pure algebra, fully testable), then `schedule_rule.rs`, `effective_schedule.rs`, `resource.rs`, `booking.rs`, `errors.rs`, `events.rs`
3. **Application ports** — `ports.rs`, `dto.rs`, `schedule_calculator.rs`
4. **SQL migrations** — in `infrastructure/src/postgres/scheduler_migrations.rs`
5. **Infrastructure repos** — `resource_repo.rs` → `schedule_rule_repo.rs` → `effective_interval_store.rs` → `booking_repo.rs`
6. **Application commands** — CreateResource → CreateScheduleRule → UpdateScheduleRule → CreateBooking → CancelBooking
7. **Application queries** — GetAvailableSlots, GetResourceSchedule, ListResources
8. **API handlers** — wire into `state.rs` and `main.rs`, add routes, update openapi

---

## Wiring into `main.rs` / `AppState`

```rust
// state.rs — add fields:
pub create_resource: Arc<CreateResourceHandler>,
pub create_schedule_rule: Arc<CreateScheduleRuleHandler>,
pub update_schedule_rule: Arc<UpdateScheduleRuleHandler>,
pub create_booking: Arc<CreateBookingHandler>,
pub cancel_booking: Arc<CancelBookingHandler>,
pub get_available_slots: Arc<GetAvailableSlotsHandler>,
pub get_resource_schedule: Arc<GetResourceScheduleHandler>,

// main.rs — after existing DB setup:
run_scheduler_migrations(&pool).await?;
let resource_repo = Arc::new(PgResourceRepository::new(pool.clone()));
let rule_repo = Arc::new(PgScheduleRuleRepository::new(pool.clone()));
let interval_store = Arc::new(PgEffectiveIntervalStore::new(pool.clone()));
let booking_repo = Arc::new(PgBookingRepository::new(pool.clone()));
// ... build handlers, add to AppState
```

---

## Verification

1. **Unit tests** (no DB): `TimeInterval` algebra operations in `domain/src/scheduler/time_interval.rs` — test union, intersect, subtract edge cases
2. **Integration tests**: Start PostgreSQL, run migrations, test:
   - Create resource hierarchy (office → room → doctor)
   - Add weekly availability rule to office, specific break rule to room
   - Verify effective schedule of doctor is intersection of all three
   - Create booking in available slot → succeeds
   - Attempt duplicate booking exceeding capacity → rejected
   - Attempt to update rule to exclude existing booking → rejected with conflict list
   - Cancel booking, retry rule update → succeeds
3. **Load test**: Use `k6` or `wrk` to hit `POST /bookings` concurrently with same resource/time → verify only `max_concurrent_events` succeed, rest return 409 Conflict
4. **OpenAPI**: Check `/scalar` renders all new scheduler endpoints

---

## Status: COMPLETE ✅

All items from this plan have been implemented. Summary of what was built:

**Domain:** `time_interval.rs` (with union/intersect/subtract algebra), `schedule_rule.rs`, `effective_schedule.rs`, `resource.rs`, `booking.rs`, `errors.rs`, `events.rs`

**Application:** Ports (`ResourceRepository`, `ScheduleRuleRepository`, `BookingRepository`, `EffectiveIntervalStore`), DTOs, `schedule_utils.rs` (compute + rematerialize), all 6 commands (CreateResource, CreateScheduleRule, UpdateScheduleRule, DeleteScheduleRule, CreateBooking, CancelBooking), all 5 queries (ListResources, GetResource, GetResourceSchedule, GetAvailableSlots, GetBooking)

**Infrastructure:** SQL migrations for all 5 tables, `PgResourceRepository` (with recursive CTE for ancestors), `PgScheduleRuleRepository` (JSONB parameters), `PgEffectiveIntervalStore` (replace-in-range), `PgBookingRepository` (advisory locks + capacity check in transaction)

**API:** All 11 routes wired, full OpenAPI documentation via utoipa + Scalar at `/scalar`

**Tests:**
- 10 unit tests in `domain` (TimeInterval algebra + EffectiveSchedule)
- 3 integration tests in `infrastructure/tests/scheduler_integration.rs` (require PostgreSQL, run with `cargo test -p infrastructure -- --ignored --test-threads=1`)

**Load testing** (not automated — use k6 or wrk manually against running server):
```
POST /bookings with same resource/time from concurrent workers
→ only max_concurrent_events succeed, rest return 409 Conflict
```
