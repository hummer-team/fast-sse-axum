pub mod redis_stream {
    use crate::message_package::message_package::EventPackage;
    use crate::redis_pool::redis_pool;
    use crate::sse_service::sse_service;
    use bb8_redis::bb8::Pool;
    use bb8_redis::bb8::PooledConnection;
    use bb8_redis::RedisConnectionManager;
    use redis::{streams::StreamReadReply, AsyncCommands, RedisResult};
    use redis::{ErrorKind, RedisError};
    use serde_json;
    use std::sync::OnceLock;
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;
    use tracing::{debug, error, info, warn};

    // Redis Stream
    pub const DEFAULT_STREAM_NAME: &str = "sse:events";
    pub const DEFAULT_GROUP_NAME: &str = "sse_consumers";
    pub const CONSUMER_NAME_PREFIX: &str = "sse-server";

    pub type RedisPool = Pool<RedisConnectionManager>;

    /// Initialize redis connection pool
    pub async fn init() -> Result<(), Box<dyn std::error::Error>> {
        // init redis connection pool
        let redis_cnn = redis_pool::create_redis_pool()
            .await
            .expect("Failed to initialize Redis connection pool");
        // ensure consumer group
        ensure_consumer_group(&redis_cnn).await?;
        let shutdown = CancellationToken::new();
        let listener_shutdown = shutdown.clone();
        // start stream listener
        tokio::spawn(async move { run_stream_listener(redis_cnn, listener_shutdown).await });
        Ok(())
    }

    /// ensure consumer group
    pub async fn ensure_consumer_group(pool: &RedisPool) -> RedisResult<()> {
        let mut conn = redis_pool::get_conn(pool).await?;
        let r: RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(stream_name())
            .arg(group_name())
            .arg("$")
            .arg("MKSTREAM")
            .query_async(&mut *conn)
            .await;
        match r {
            Ok(..) => {
                info!(
                    "created group success stream name {} group name {}",
                    stream_name(),
                    group_name()
                );
                Ok(())
            }
            Err(e) => {
                if is_busygroup_error(&e) {
                    info!(
                        "Consumer group {} already exists for stream {}",
                        group_name(),
                        stream_name()
                    );
                    Ok(())
                } else {
                    error!(
                        "created group fail stream name {} group name {} error {}",
                        stream_name(),
                        group_name(),
                        e
                    );
                    Err(e)
                }
            }
        }
    }

    fn is_busygroup_error(err: &RedisError) -> bool {
        if let Some(code) = err.code() {
            return code == "BUSYGROUP";
        }
        matches!(err.kind(), ErrorKind::ResponseError) && err.to_string().contains("BUSYGROUP")
    }

    /// parse stream message to MessagePackage
    fn parse_stream_message(
        id: &str,
        fields: &std::collections::HashMap<String, redis::Value>,
    ) -> Result<EventPackage, Box<dyn std::error::Error + Send + Sync>> {
        for (key, value) in fields.iter() {
            debug!("origin message key: {}, value: {:?}", key, value);
        }
        let (_key, value) = fields.iter().next().ok_or("No fields")?;

        let bytes = match value {
            redis::Value::BulkString(b) => b,
            // redis::Value::Status(s) => s,
            _ => return Err(format!("Invalid value type: {:?}", value).into()),
        };

        serde_json::from_slice::<EventPackage>(bytes)
            .map_err(|e| format!("JSON deserialization error for message {}: {}", id, e).into())
    }

    async fn process_stream_entries(
        conn: &mut PooledConnection<'_, RedisConnectionManager>,
        entries: &[redis::streams::StreamKey],
    ) -> RedisResult<()> {
        // let mut conn = redis_pool::get_conn(redis_pool).await?;
        for stream_key in entries {
            info!(
                "Processing stream entry: {:?}, stream message {:?}",
                stream_key.key, stream_key.ids
            );
            if stream_key.key != *stream_name() {
                continue;
            }
            for stream_id in &stream_key.ids {
                let msg_id = &stream_id.id;
                // parse message
                match parse_stream_message(msg_id, &stream_id.map) {
                    // send message
                    Ok(msg) => match sse_service::send_message(msg).await {
                        Ok(..) => {
                            let _: RedisResult<i32> =
                                conn.xack(stream_name(), group_name(), &[msg_id]).await;
                            info!(
                                "send message success, ack message success msg id :{}",
                                msg_id
                            );
                        }
                        Err(e) => {
                            error!("send message error: {:?}", e);
                            let def = ("NA".to_string(), "NA".to_string());
                            if e.unwrap_or(def).0 == "NO_ACTIVE_SUBSCRIPTION" {
                                let _: RedisResult<i32> =
                                    conn.xack(stream_name(), group_name(), &[msg_id]).await;
                                warn!(
                                    "no active subscription, ack message success msg id :{}",
                                    msg_id
                                );
                            }
                        }
                    },
                    // :? debug format print log
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
        let consumer_name = format!("{}:{}", CONSUMER_NAME_PREFIX, std::process::id());
        info!(
            "Starting Redis stream listener with consumer name: {}",
            consumer_name
        );

        if let Ok(mut conn) = redis_pool::get_conn(&redis_pool).await {
            info!("Checking for pending messages...");
            match read_stream_messages(&mut conn, &consumer_name, true).await {
                Ok(reply) if !reply.keys.is_empty() => {
                    if let Err(e) = process_stream_entries(&mut conn, &reply.keys).await {
                        error!("Failed to process pending stream entries: {}", e);
                    }
                }
                Err(e) => error!("Failed to read pending messages: {}", e),
                _ => info!("No pending messages found."),
            }
        }

        loop {
            let mut conn = match redis_pool::get_conn(&redis_pool).await {
                Ok(c) => c,
                Err(e) => {
                    error!(
                        "Failed to get Redis connection from pool: {}. Retrying in 5s.",
                        e
                    );
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("Redis stream listener shutting down");
                    break;
                }
                result = read_stream_messages(&mut conn, &consumer_name, false) => {
                    match result {
                        Ok(reply) => {
                            // handler message
                            if !reply.keys.is_empty() {
                                if let Err(e) = process_stream_entries(&mut conn, &reply.keys).await {
                                    error!("Failed to process stream entries: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to read stream messages: {}", e);
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn read_stream_messages(
        conn: &mut PooledConnection<'_, RedisConnectionManager>,
        consumer_name: &str,
        read_pending: bool,
    ) -> RedisResult<StreamReadReply> {
        let opts = redis::streams::StreamReadOptions::default()
            .group(group_name(), consumer_name)
            .count(10)
            .block(1000);
        let stream_id = if read_pending {
            // "0" reads all pending messages for this consumer that were not acknowledged.
            "0"
        } else {
            // ">" reads new messages that have not been delivered to any consumer in the group.
            ">"
        };
        // read messages
        conn.xread_options(&[stream_name()], &[stream_id], &opts)
            .await
    }

    fn stream_name() -> &'static String {
        static STREAM_NAME: OnceLock<String> = OnceLock::new();
        STREAM_NAME.get_or_init(|| {
            std::env::var("REDIS_STREAM_NAME").unwrap_or_else(|_| DEFAULT_STREAM_NAME.to_string())
        })
    }

    fn group_name() -> &'static String {
        static GROUP_NAME: OnceLock<String> = OnceLock::new();
        GROUP_NAME.get_or_init(|| {
            std::env::var("REDIS_GROUP_NAME").unwrap_or_else(|_| DEFAULT_GROUP_NAME.to_string())
        })
    }
}
