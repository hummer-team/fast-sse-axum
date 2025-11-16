pub mod redis_stream {
    use crate::message_package::message_package::EventPackage;
    use crate::redis_pool::redis_pool;
    use crate::sse_service::sse_service;
    use bb8_redis::bb8::Pool;
    use bb8_redis::RedisConnectionManager;
    use lazy_static::lazy_static;
    use redis::{streams::StreamReadReply, AsyncCommands, RedisResult};
    use serde_json;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;
    use tracing::{error, info};

    // Redis Stream
    pub const DEFAULT_STREAM_NAME: &str = "sse:events";
    pub const DEFAULT_GROUP_NAME: &str = "sse_consumers";
    pub const CONSUMER_NAME_PREFIX: &str = "sse-server";
    pub const STREAM_MESSAGE_FIELD: &str = "message";

    pub type RedisPool = Arc<Pool<RedisConnectionManager>>;

    lazy_static! {
        pub static ref stream_name: String =
            std::env::var("REDIS_STREAM_NAME").unwrap_or(DEFAULT_STREAM_NAME.to_string());
        pub static ref group_name: String =
            std::env::var("REDIS_GROUP_NAME").unwrap_or(DEFAULT_GROUP_NAME.to_string());
    }

    /// Initialize redis connection pool
    pub async fn init() {
        // init redis connection pool
        let redis_cnn = redis_pool::create_redis_pool().await;
        // ensure consumer group
        let _ = ensure_consumer_group(&redis_cnn).await;
        let shutdown = CancellationToken::new();
        let listener_shutdown = shutdown.clone();
        // start stream listener
        tokio::spawn(async move { run_stream_listener(redis_cnn, listener_shutdown).await });
    }

    /// ensure consumer group
    pub async fn ensure_consumer_group(pool: &RedisPool) -> RedisResult<()> {
        let mut conn = redis_pool::get_conn(pool).await?;
        let r: RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(stream_name.as_str())
            .arg(group_name.as_str())
            .arg("$")
            .arg("MKSTREAM")
            .query_async(&mut *conn)
            .await;
        match r {
            Ok(..) => {
                info!(
                    "created group success stream name {} group name {}",
                    stream_name.as_str(),
                    group_name.as_str()
                );
            }
            Err(e) => {
                error!(
                    "created group fail stream name {} group name {} error {}",
                    stream_name.as_str(),
                    group_name.as_str(),
                    e
                );
                // panic sys exception
                panic!(
                    "created group fail stream name {} group name {} error {}",
                    stream_name.as_str(),
                    group_name.as_str(),
                    e
                );
            }
        }
        Ok(())
    }

    /// parse stream message to MessagePackage
    fn parse_stream_message(
        id: &str,
        fields: &std::collections::HashMap<String, redis::Value>,
    ) -> Result<EventPackage, String> {
        let redis_value = fields
            .get(STREAM_MESSAGE_FIELD)
            .ok_or_else(|| format!("Missing '{}' field in message {}", STREAM_MESSAGE_FIELD, id))?;

        // convert Redis Value to string
        let json_str: String = redis::from_redis_value(redis_value)
            .map_err(|e| format!("Failed to convert Redis value to string for {}: {}", id, e))?;

        serde_json::from_str::<EventPackage>(&json_str).map_err(|e| {
            format!(
                "JSON parse error for message {}: {}. Raw: {}",
                id, e, json_str
            )
        })
    }

    async fn process_stream_entries(
        redis_pool: &RedisPool,
        entries: &[redis::streams::StreamKey],
    ) -> RedisResult<()> {
        let mut conn = redis_pool::get_conn(redis_pool).await?;
        for stream_key in entries {
            if stream_key.key != stream_name.to_string() {
                continue;
            }
            for stream_id in &stream_key.ids {
                let msg_id = &stream_id.id;
                // parse message
                match parse_stream_message(msg_id, &stream_id.map) {
                    // send message
                    Ok(msg) => match sse_service::send_message(msg).await {
                        Ok(..) => {
                            let _: RedisResult<i32> = conn
                                .xack(stream_name.as_str(), group_name.as_str(), &[msg_id])
                                .await;
                        }
                        Err(e) => error!("send message error: {:?}", e),
                    },
                    Err(e) => error!("get message error: {:?}", e),
                }
            }
        }
        Ok(())
    }

    pub async fn run_stream_listener(
        redis_pool: RedisPool,
        shutdown: CancellationToken,
    ) -> RedisResult<()> {
        let consumer_name = format!("{}:{}", CONSUMER_NAME_PREFIX, "1");
        let mut read_pending = true;
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("Redis stream listener shutting down");
                    break;
                }
                result = read_stream_messages(&redis_pool, &consumer_name, read_pending) => {
                    match result {
                        Ok(reply) => {
                            read_pending = false;
                            // handler message
                            if let Err(e) = process_stream_entries(&redis_pool, &reply.keys).await {
                                error!("Failed to process stream entries: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to read stream messages: {}", e);
                            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn read_stream_messages(
        redis_pool: &RedisPool,
        consumer_name: &str,
        read_pending: bool,
    ) -> RedisResult<StreamReadReply> {
        let mut conn = redis_pool::get_conn(redis_pool).await?;
        // let mut conn = pool.get_multiplexed_tokio_connection().await?;
        let opts = redis::streams::StreamReadOptions::default()
            .group(group_name.as_str(), consumer_name)
            .count(10)
            .block(1000);

        if read_pending {
            let _: RedisResult<Vec<redis::Value>> = redis::cmd("XPENDING")
                .arg(stream_name.as_str())
                .arg(group_name.as_str())
                .arg("0")
                .arg("+")
                .arg("10")
                .query_async(&mut *conn)
                .await;
        }
        conn.xread_options(&[stream_name.as_str()], &[">"], &opts)
            .await
    }
}
