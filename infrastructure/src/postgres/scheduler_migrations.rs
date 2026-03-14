use sqlx::PgPool;

pub async fn run_scheduler_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS resources (
            id                       UUID PRIMARY KEY,
            name                     VARCHAR(200) NOT NULL,
            parent_id                UUID REFERENCES resources(id) ON DELETE RESTRICT,
            max_concurrent_events    INT NOT NULL DEFAULT 1 CHECK (max_concurrent_events >= 1),
            inherits_parent_schedule BOOLEAN NOT NULL DEFAULT false,
            created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_resources_parent_id ON resources(parent_id)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
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
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_schedule_rules_resource ON schedule_rules(resource_id, rule_kind)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS effective_intervals (
            id                 BIGSERIAL PRIMARY KEY,
            resource_id        UUID NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
            date               DATE NOT NULL,
            start_time         TIME NOT NULL,
            end_time           TIME NOT NULL,
            available_capacity INT NOT NULL,
            computed_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_effective_intervals_resource_date_time
            ON effective_intervals(resource_id, date, start_time)
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS bookings (
            id         UUID PRIMARY KEY,
            start_at   TIMESTAMPTZ NOT NULL,
            end_at     TIMESTAMPTZ NOT NULL,
            status     VARCHAR(20) NOT NULL DEFAULT 'confirmed' CHECK (status IN ('confirmed','cancelled')),
            metadata   JSONB,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT bookings_time_check CHECK (end_at > start_at)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS booking_resources (
            booking_id  UUID NOT NULL REFERENCES bookings(id) ON DELETE CASCADE,
            resource_id UUID NOT NULL REFERENCES resources(id) ON DELETE RESTRICT,
            start_at    TIMESTAMPTZ NOT NULL,
            end_at      TIMESTAMPTZ NOT NULL,
            PRIMARY KEY (booking_id, resource_id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_booking_resources_resource_time
            ON booking_resources(resource_id, start_at, end_at)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
