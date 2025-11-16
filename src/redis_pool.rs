pub mod redis_pool {
    use bb8_redis::bb8::{Pool, PooledConnection};
    use bb8_redis::RedisConnectionManager;
    use std::sync::Arc;
    use tracing::info;

    pub type RedisPool = Arc<Pool<RedisConnectionManager>>;

    /// create redis pool
    pub async fn create_redis_pool() -> RedisPool {
        let max_connections = std::env::var("REDIS_MAX_CONNECTIONS_SSE")
            .unwrap_or("10".to_string())
            .parse::<u32>()
            .unwrap();
        let min_idle = std::env::var("min_idle")
            .unwrap_or("3".to_string())
            .parse::<u32>()
            .unwrap();
        let connection_timeout = std::time::Duration::from_secs(
            std::env::var("connection_timeout")
                .unwrap_or("10".to_string())
                .parse::<u64>()
                .unwrap(),
        );
        let idle_timeout = std::time::Duration::from_secs(
            std::env::var("idle_timeout")
                .unwrap_or("10".to_string())
                .parse::<u64>()
                .unwrap(),
        );
        let max_lifetime = std::time::Duration::from_secs(
            std::env::var("max_lifetime")
                .unwrap_or("10".to_string())
                .parse::<u64>()
                .unwrap(),
        );
        let redis_url = std::env::var("REDIS_URL_SSE").unwrap();
        let manager = RedisConnectionManager::new(redis_url.as_str()).expect("Invalid Redis URL");
        let pool = Pool::builder()
            .max_size(max_connections)
            .min_idle(Some(min_idle))
            .connection_timeout(connection_timeout)
            .idle_timeout(Some(idle_timeout))
            .max_lifetime(Some(max_lifetime))
            .test_on_check_out(true)
            .build(manager)
            .await
            .expect("Failed to create Redis pool");

        info!("Redis pool created {}", redis_url);

        Arc::new(pool)
    }

    /// Get a connection from the pool
    pub async fn get_conn(
        pool: &RedisPool,
    ) -> Result<PooledConnection<'_, RedisConnectionManager>, redis::RedisError> {
        pool.get().await.map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::IoError,
                "Failed to get connection from pool",
                e.to_string(),
            ))
        })
    }
}
