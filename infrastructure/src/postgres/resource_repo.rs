use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use application::errors::AppError;
use application::scheduler::ports::ResourceRepository;
use domain::scheduler::Resource;

pub struct PgResourceRepository {
    pool: PgPool,
}

impl PgResourceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ResourceRow {
    id: Uuid,
    name: String,
    parent_id: Option<Uuid>,
    max_concurrent_events: i32,
    inherits_parent_schedule: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ResourceRow> for Resource {
    fn from(r: ResourceRow) -> Self {
        Resource::from_raw(
            r.id,
            r.name,
            r.parent_id,
            r.max_concurrent_events,
            r.inherits_parent_schedule,
            r.created_at,
            r.updated_at,
        )
    }
}

#[async_trait]
impl ResourceRepository for PgResourceRepository {
    async fn save(&self, resource: &Resource) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO resources (id, name, parent_id, max_concurrent_events, inherits_parent_schedule, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(resource.id)
        .bind(&resource.name)
        .bind(resource.parent_id)
        .bind(resource.max_concurrent_events)
        .bind(resource.inherits_parent_schedule)
        .bind(resource.created_at)
        .bind(resource.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Resource>, AppError> {
        sqlx::query_as::<_, ResourceRow>(
            "SELECT id, name, parent_id, max_concurrent_events, inherits_parent_schedule, created_at, updated_at FROM resources WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map(|opt| opt.map(Resource::from))
        .map_err(|e| AppError::Database(e.to_string()))
    }

    async fn find_children(&self, parent_id: Uuid) -> Result<Vec<Resource>, AppError> {
        sqlx::query_as::<_, ResourceRow>(
            "SELECT id, name, parent_id, max_concurrent_events, inherits_parent_schedule, created_at, updated_at FROM resources WHERE parent_id = $1",
        )
        .bind(parent_id)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(Resource::from).collect())
        .map_err(|e| AppError::Database(e.to_string()))
    }

    async fn find_ancestors(&self, id: Uuid) -> Result<Vec<Resource>, AppError> {
        // Returns chain from nearest parent to root
        sqlx::query_as::<_, ResourceRow>(
            r#"
            WITH RECURSIVE ancestors AS (
                SELECT r.id, r.name, r.parent_id, r.max_concurrent_events,
                       r.inherits_parent_schedule, r.created_at, r.updated_at
                FROM resources r
                WHERE r.id = (SELECT parent_id FROM resources WHERE id = $1)
                UNION ALL
                SELECT r.id, r.name, r.parent_id, r.max_concurrent_events,
                       r.inherits_parent_schedule, r.created_at, r.updated_at
                FROM resources r
                JOIN ancestors a ON r.id = a.parent_id
            )
            SELECT id, name, parent_id, max_concurrent_events, inherits_parent_schedule, created_at, updated_at
            FROM ancestors
            "#,
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(Resource::from).collect())
        .map_err(|e| AppError::Database(e.to_string()))
    }

    async fn list_all(&self) -> Result<Vec<Resource>, AppError> {
        sqlx::query_as::<_, ResourceRow>(
            "SELECT id, name, parent_id, max_concurrent_events, inherits_parent_schedule, created_at, updated_at FROM resources ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(Resource::from).collect())
        .map_err(|e| AppError::Database(e.to_string()))
    }
}
