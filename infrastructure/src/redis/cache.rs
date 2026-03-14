use async_trait::async_trait;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;

use application::errors::AppError;
use application::ports::CachePort;

// ============================================================
// Redis Cache — реализация CachePort.
// ConnectionManager — автоматический reconnect, потокобезопасный.
// В C# аналог: IDistributedCache + StackExchange.Redis.
// ============================================================

#[derive(Clone)]
pub struct RedisCache {
    conn: ConnectionManager,
}

impl RedisCache {
    pub async fn new(redis_url: &str) -> Result<Self, AppError> {
        let client =
            redis::Client::open(redis_url).map_err(|e| AppError::Cache(e.to_string()))?;
        let conn = ConnectionManager::new(client)
            .await
            .map_err(|e| AppError::Cache(e.to_string()))?;

        Ok(Self { conn })
    }
}

#[async_trait]
impl CachePort for RedisCache {
    async fn get_raw(&self, key: &str) -> Result<Option<Vec<u8>>, AppError> {
        let mut conn = self.conn.clone();
        let value: Option<Vec<u8>> = conn
            .get(key)
            .await
            .map_err(|e| AppError::Cache(e.to_string()))?;
        Ok(value)
    }

    async fn set_raw(&self, key: &str, value: &[u8], ttl_secs: u64) -> Result<(), AppError> {
        let mut conn = self.conn.clone();
        conn.set_ex::<_, _, ()>(key, value, ttl_secs)
            .await
            .map_err(|e| AppError::Cache(e.to_string()))?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), AppError> {
        let mut conn = self.conn.clone();
        conn.del::<_, ()>(key)
            .await
            .map_err(|e| AppError::Cache(e.to_string()))?;
        Ok(())
    }

    async fn delete_pattern(&self, pattern: &str) -> Result<(), AppError> {
        let mut conn = self.conn.clone();

        // SCAN + DEL — безопаснее чем KEYS в продакшене
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Cache(e.to_string()))?;

        if !keys.is_empty() {
            for key in &keys {
                let _: () = conn
                    .del(key)
                    .await
                    .map_err(|e| AppError::Cache(e.to_string()))?;
            }
        }

        Ok(())
    }
}
