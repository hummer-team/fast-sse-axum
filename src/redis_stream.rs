use crate::message_package::EventPackage;
use crate::redis_pool;
use crate::sse_service;
use bb8_redis::RedisConnectionManager;
use bb8_redis::bb8::Pool;
use bb8_redis::bb8::PooledConnection;
use redis::{AsyncCommands, RedisResult, streams::StreamReadReply};
use redis::{ErrorKind, RedisError};
use serde_json;
use std::sync::OnceLock;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

// Redis Stream
const DEFAULT_STREAM_NAME: &str = "sse:events";
const DEFAULT_GROUP_NAME: &str = "sse_consumers";

type RedisPool = Pool<RedisConnectionManager>;
static REDIS_POOL: OnceLock<Pool<RedisConnectionManager>> = OnceLock::new();

/// Initialize redis connection pool
pub async fn init() -> Result<(), Box<dyn std::error::Error>> {
    // init redis connection pool
    let redis_pool = redis_pool::create_redis_pool().await?;
    // ensure consumer group
    ensure_consumer_group(&redis_pool).await?;
    let shutdown = CancellationToken::new();
    let listener_shutdown = shutdown.clone();
    // start stream listener
    let pool_for_listener = redis_pool.clone();
    tokio::spawn(async move { run_stream_listener(pool_for_listener, listener_shutdown).await });
    REDIS_POOL
        .set(redis_pool)
        .expect("Failed to set Redis pool");
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

/// Publish event to redis stream
pub async fn publish_event(event: EventPackage) -> RedisResult<String> {
    // Get the pool from the global static
    let pool = REDIS_POOL.get().expect("Redis pool not initialized");
    let mut conn = redis_pool::get_conn(pool).await?;
    // Serialize the event package
    let msg_json = serde_json::to_string(&event).map_err(|e| {
        redis::RedisError::from((
            redis::ErrorKind::TypeError,
            "Serialization failed",
            e.to_string(),
        ))
    })?;

    redis::cmd("XADD")
        .arg(stream_name())
        .arg("*")
        .arg(event.event_id)
        .arg(msg_json)
        .query_async(&mut *conn)
        .await
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
    for stream_key in entries {
        debug!("processing stream entry: {:?}", stream_key.key);
        if stream_key.key != *stream_name() {
            continue;
        }
        for stream_id in &stream_key.ids {
            let msg_id = &stream_id.id;
            // parse message
            match parse_stream_message(msg_id, &stream_id.map) {
                // send message
                Ok(msg) => {
                    let dlq_handler = |event_package: EventPackage| async move {
                        if let Err(e) = push_to_dead_letter_queue(&event_package).await {
                            error!("Failed to push message to dead-letter queue: {}", e);
                        }
                    };
                    // Delegate the sending task. This call returns immediately.
                    sse_service::send_mesage_with_no_block(msg, dlq_handler).await;
                    // As per requirement, ACK the message immediately to unblock the stream.
                    // The spawned task is now responsible for delivery or DLQ.
                    let _: RedisResult<i32> =
                        conn.xack(stream_name(), group_name(), &[msg_id]).await;
                    info!(
                        "Delegated message for processing and ACKed msg_id: {}",
                        msg_id
                    );
                }
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
    let consumer_name = group_name();
    info!(
        "Starting Redis stream listener with consumer name: {}",
        group_name()
    );

    if let Ok(mut conn) = redis_pool::get_conn(&redis_pool).await {
        info!("Checking for pending messages...");
        match read_stream_messages(&mut conn, consumer_name, true).await {
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

/// Pushes a failed event package to the dead-letter queue.
pub async fn push_to_dead_letter_queue(message: &EventPackage) -> RedisResult<()> {
    let pool = REDIS_POOL.get().expect("Redis pool not initialized");
    let mut conn = redis_pool::get_conn(pool).await?;

    let msg_json = serde_json::to_string(message).map_err(|e| {
        redis::RedisError::from((
            redis::ErrorKind::TypeError,
            "DLQ Serialization failed",
            e.to_string(),
        ))
    })?;

    let dlq_stream_name = format!("{}:dead_letter", stream_name());

    info!(
        "Pushing message for event_id: {} to DLQ stream: {}",
        message.get_event_id(),
        dlq_stream_name
    );

    redis::cmd("XADD")
        .arg(&dlq_stream_name)
        .arg("*")
        .arg(message.get_event_id())
        .arg(msg_json)
        .query_async(&mut *conn)
        .await
}

async fn read_stream_messages(
    conn: &mut PooledConnection<'_, RedisConnectionManager>,
    consumer_name: &str,
    read_pending: bool,
) -> RedisResult<StreamReadReply> {
    let opts = redis::streams::StreamReadOptions::default()
        .group(group_name(), consumer_name)
        .count(10)
        .block(200);
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
    // only init once
    static STREAM_NAME: OnceLock<String> = OnceLock::new();
    STREAM_NAME.get_or_init(|| {
        std::env::var("QUEUE_NAME").unwrap_or_else(|_| DEFAULT_STREAM_NAME.to_string())
    })
}

fn group_name() -> &'static String {
    static GROUP_NAME: OnceLock<String> = OnceLock::new();
    GROUP_NAME.get_or_init(|| {
        let name = std::env::var("CONSUMERS_GROUP_NAME")
            .unwrap_or_else(|_| DEFAULT_GROUP_NAME.to_string());
        let id =
            std::env::var("CONSUMERS_GROUP_ID").unwrap_or_else(|_| std::process::id().to_string());
        format!("{}_{}", name, id)
    })
}
