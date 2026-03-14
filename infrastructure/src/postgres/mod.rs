pub mod booking_repo;
pub mod effective_interval_store;
pub mod resource_repo;
pub mod schedule_rule_repo;
mod scheduler_migrations;

pub use booking_repo::PgBookingRepository;
pub use effective_interval_store::PgEffectiveIntervalStore;
pub use resource_repo::PgResourceRepository;
pub use schedule_rule_repo::PgScheduleRuleRepository;

use sqlx::postgres::PgPoolOptions;
use sqlx::{Connection, PgConnection, PgPool};

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    ensure_database_exists(database_url).await?;

    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

async fn ensure_database_exists(database_url: &str) -> Result<(), sqlx::Error> {
    let (admin_url, db_name) = split_database_url(database_url);
    let mut conn = PgConnection::connect(&admin_url).await?;

    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(&db_name)
            .fetch_one(&mut conn)
            .await?;

    if !exists {
        sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
            .execute(&mut conn)
            .await?;
    }

    Ok(())
}

fn split_database_url(database_url: &str) -> (String, String) {
    if let Some(pos) = database_url.rfind('/') {
        let db_name = database_url[pos + 1..].to_string();
        let db_name = db_name.split('?').next().unwrap_or(&db_name).to_string();
        let admin_url = format!("{}/postgres", &database_url[..pos]);
        (admin_url, db_name)
    } else {
        (database_url.to_string(), String::new())
    }
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    scheduler_migrations::run_scheduler_migrations(pool).await
}
