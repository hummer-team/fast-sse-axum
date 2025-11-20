pub mod redis_pool {
    use bb8_redis::bb8::{Pool, PooledConnection, RunError};
    use bb8_redis::RedisConnectionManager;
    use std::env;
    use std::str::FromStr;
    use std::time::Duration;
    use tracing::info;

    pub type RedisPool = Pool<RedisConnectionManager>;

    /// create redis pool
    pub async fn create_redis_pool() -> Result<RedisPool, Box<dyn std::error::Error>> {
        let max_connections = get_env_var::<u32>("REDIS_POOL_MAX_SIZE", "10")?;
        let min_idle = get_env_var::<u32>("REDIS_POOL_MIN_IDLE", "3")?;
        let connection_timeout_secs = get_env_var::<u64>("REDIS_POOL_CONNECTION_TIMEOUT", "10")?;
        let idle_timeout_secs = get_env_var::<u64>("REDIS_POOL_IDLE_TIMEOUT", "60")?;
        let max_lifetime_secs = get_env_var::<u64>("REDIS_POOL_MAX_LIFETIME", "1800")?;
        let redis_url = env::var("REDIS_URL_SSE").map_err(|_| "REDIS_URL_SSE must be set")?;

        let manager = RedisConnectionManager::new(redis_url.as_str()).expect("Invalid Redis URL");
        let pool = Pool::builder()
            .max_size(max_connections)
            .min_idle(Some(min_idle))
            .connection_timeout(Duration::from_secs(connection_timeout_secs))
            .idle_timeout(Some(Duration::from_secs(idle_timeout_secs)))
            .max_lifetime(Some(Duration::from_secs(max_lifetime_secs)))
            .test_on_check_out(true)
            .build(manager)
            .await
            .expect("Failed to create Redis pool");

        info!("Redis pool created {}", redis_url);

        Ok(pool)
    }

    /// Get a connection from the pool
    pub async fn get_conn(
        pool: &RedisPool,
    ) -> Result<PooledConnection<'_, RedisConnectionManager>, redis::RedisError> {
        pool.get()
            .await
            .map_err(|e: RunError<redis::RedisError>| match e {
                RunError::User(inner) => inner,
                RunError::TimedOut => redis::RedisError::from((
                    redis::ErrorKind::IoError,
                    "Timeout getting connection from pool",
                )),
            })
    }

    fn get_env_var<T: FromStr>(name: &str, default: &str) -> Result<T, String> {
        let value_str = env::var(name).unwrap_or_else(|_| default.to_string());
        value_str.parse::<T>().map_err(|_| {
            format!(
                "Failed to parse env var '{}' with value '{}'",
                name, value_str
            )
        })
    }
}
