pub mod sse_service {
    use crate::auth_middle::auth_middle::{AuthConfig, auth};
    use crate::common::sse_common::sse_common::get_env_var;
    use crate::message_compression::message_compression;
    use crate::message_package::message_package::EventPackage;
    use crate::response_builder::response_builder::ResponseBuilder;
    use crate::sse_event_publish::sse_event_publish;
    use crate::sse_event_publish::sse_event_publish::EventPublisher;
    use crate::{
        common::sse_common::sse_common::ResourceResponse,
        common::sse_common::sse_common::now_time_with_format,
    };
    use axum::extract::Path;
    use axum::middleware::from_fn_with_state;
    use axum::{
        Json, Router,
        body::Body,
        http::StatusCode,
        response::{
            IntoResponse, Response,
            sse::{Event, KeepAlive, Sse},
        },
        routing::{get, post},
    };
    use base64::{Engine as _, engine::general_purpose};
    use dashmap::DashMap;
    use serde::Deserialize;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::OnceLock;
    use std::{
        convert::Infallible,
        path::PathBuf,
        time::{Duration, Instant},
    };
    use tokio::sync::broadcast;
    use tower_http::{services::ServeDir, trace::TraceLayer};
    use tracing::log::{debug, error, info, warn};

    /// SSE service enter
    pub fn init_router() -> Result<Router, Box<dyn std::error::Error>> {
        let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
        info!("Serving static files from {}", assets_dir.display());
        let static_files_service = ServeDir::new(assets_dir).append_index_html_on_directories(true);

        let allowed_ids = get_env_var::<String>("ALLOWED_EVENT_IDS", Some(""))?
            .split(",")
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        let allowed_events = get_env_var::<String>("ALLOWED_EVENT_TYPES", Some(""))?
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        let secret = get_env_var::<String>("HMAC_SECRET", None)?.into_bytes();
        let config = Arc::new(AuthConfig::new(allowed_ids, secret, allowed_events));

        // build our application with a route
        let router = Router::new()
            .route(
                "/v1/subscribe/events/{event_id}/types/{event_type}",
                get(subscribert), // http method is get
            )
            .route(
                "/v1/sse/events/{event_id}/types/{event_type}",
                post(send_message_with_web_request), // http method is post
            )
            .layer(from_fn_with_state(config.into(), auth));

        // only for debug
        let router = if cfg!(debug_assertions) {
            info!("Registering mock test producer event endpoint for debug builds");
            router.route("/v1/sse/events", post(mock_test_producer_event))
        } else {
            router
        }
        .fallback_service(static_files_service)
        .layer(TraceLayer::new_for_http());
        info!("sse router register success");
        Ok(router)
    }

    struct ChannelMeta {
        sender: broadcast::Sender<Arc<String>>,
        last_activity: Mutex<Instant>,
    }

    /// Each event_id corresponds to a channel, and all clients subscribed to that event_id share that channel.
    static CLIENT_SUBSCRIPTIONS: OnceLock<DashMap<String, ChannelMeta>> = OnceLock::new();

    const CLEANUP_INTERVAL: Duration = Duration::from_secs(45);
    const CHANNEL_TTL: Duration = Duration::from_secs(120);

    /// init
    pub async fn init() {
        let _ = CLIENT_SUBSCRIPTIONS.set(DashMap::new());
        info!("SSE service subscript initialized");
        tokio::spawn(cleanup_inactive_clients());
    }

    #[derive(Deserialize)]
    #[cfg(debug_assertions)]
    pub struct MockParams {
        event_id: String,
        body_size: usize, // in KB
    }

    #[cfg(debug_assertions)]
    pub async fn mock_test_producer_event(Json(params): Json<MockParams>) -> Response {
        let body_size_bytes = params.body_size * 1024;
        let large_body: Vec<u8> = vec![0; body_size_bytes];
        let encoded_body = general_purpose::STANDARD.encode(&large_body);

        // Create an EventPackage with the mock data.
        let event_package = EventPackage::new(
            "mock_message".to_string(),
            params.event_id,
            "mock_from_user".to_string(),
            "mock_to_user".to_string(),
            Some(serde_json::Value::String(encoded_body)),
            None, // headers
            Some("mock_event_type".to_string()),
            Some(false),
        );
        let publish = sse_event_publish::RedisEventPublisher::new();
        match publish.publish_event(event_package).await {
            Ok(msg_id) => {
                info!("Successfully published mock event with ID: {}", msg_id);
                let response_data = ResourceResponse::ok(Some(json!({ "message_id": msg_id })));
                (StatusCode::OK, Json(response_data)).into_response()
            }
            Err(e) => {
                error!("Failed to publish mock event: {}", e);
                let response_data =
                    ResourceResponse::<serde_json::Value>::error("PUBLISH_ERROR", &e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(response_data)).into_response()
            }
        }
    }

    /// Subscribe to an event
    pub async fn subscribert(Path((event_id, event_type)): Path<(String, String)>) -> Response {
        let receiver = {
            let subs = CLIENT_SUBSCRIPTIONS.get().unwrap();
            let meta = subs.entry(event_id.clone()).or_insert_with(|| {
                let (tx, _rx) = broadcast::channel::<Arc<String>>(4096);
                info!("create channel: {} (type: {})", event_id, event_type);
                ChannelMeta {
                    sender: tx,
                    last_activity: Mutex::new(Instant::now()),
                }
            });
            meta.value().sender.subscribe()
        };
        info!(
            "client subscribe event: {} / {} (receiver: {})",
            event_id,
            event_type,
            receiver.len()
        );

        let stream = async_stream::stream! {
            let mut rx = receiver;
            loop {
                match rx.recv().await {
                    Ok(json_value) => {
                        //push message to client
                        //Use `&*json_value` to get &Value from Arc<Value>
                        //let event = Event::default()
                        //            .json_data(&*json_value)
                        //            .unwrap_or_else(|e| {
                        //                warn!("sse event serialization error: {}", e);
                        //                Event::default().data("serialization error")
                        //            });
                        //old code:
                        //let event = process_and_compress_event(&json_value);
                        let event = Event::default().data(&*json_value);
                        yield Ok::<_, Infallible>(event);
                    }
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        warn!("skip message {} count,event_id {}",count,event_id);
                        yield Ok(Event::default()
                            .event("error")
                            .json_data(serde_json::json!({
                                "code": "LAGGED",
                                "message": format!("Skipped {} messages", count)
                            }))
                            .unwrap()
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("SSE channel closed: event_id={}", event_id);
                        break;
                    }
                }
            }
        };
        Sse::new(stream)
            .keep_alive(
                KeepAlive::new()
                    .interval(Duration::from_secs(60))
                    .text("keep-alive"),
            )
            .into_response()
    }

    /// Dispatches a message for sending.
    /// Tries to send immediately. If no subscribers are found,
    /// it spawns a background task to handle retries and potential dead-lettering.
    /// This function returns immediately and does not block.
    pub async fn send_mesage_with_no_block<F, Fut>(message: EventPackage, dlq_handler: F)
    where
        F: Fn(EventPackage) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let payload_to_send = get_payload(&message);
        let payload_arc = Arc::new(payload_to_send);

        // Attempt a quick send first.
        let sse_sender = {
            let subs = CLIENT_SUBSCRIPTIONS.get().unwrap();
            subs.get(&message.get_event_id()).map(|meta| {
                // Update the last activity time.
                *meta.last_activity.lock().unwrap() = Instant::now();
                meta.sender.clone()
            })
        };

        if let Some(sender) = sse_sender {
            if sender.receiver_count() > 0 {
                match sender.send(payload_arc) {
                    Ok(_) => {
                        // Fast path successful, message sent.
                        info!(
                            "Message for event_id: {} sent to client successfully.",
                            message.get_event_id()
                        );
                        return;
                    }
                    Err(_) => {
                        // This can happen if the last receiver disconnects between the receiver_count() check and send().
                        // Log this event and fall through to the slow path for retries.
                        warn!(
                            "Fast path send failed for event_id: {}. Race condition likely. Falling back to retry task.",
                            message.get_event_id()
                        );
                    }
                }
            }
        }

        // Slow path: No active subscription or sender has no receivers.
        // Spawn a background task to handle retries.
        info!(
            "No active subscribers for event_id: {}. Spawning retry task.",
            message.get_event_id()
        );
        tokio::spawn(retry_and_dispatch_message(message, Arc::new(dlq_handler)));
    }

    pub async fn send_message_with_web_request(
        Path((event_id, event_type)): Path<(String, String)>,
        Json(message): Json<EventPackage>,
    ) -> Response<Body> {
        // move the data into a variable
        let EventPackage {
            data,
            headers,
            event_name,
            user,
            send_direct,
            ..
        } = message;
        let event = EventPackage::new(
            event_name,
            event_id,
            user.as_ref().unwrap().from_user.to_string(),
            user.as_ref().unwrap().to_user.to_string(),
            data,
            headers,
            Some(event_type),
            send_direct,
        );
        debug!(
            "sending message to client message id: {} message type: {}",
            event.get_event_id(),
            event.get_event_type()
        );
        match event.send_direct {
            Some(false) => {
                let publish = sse_event_publish::RedisEventPublisher::new();
                match publish.publish_event(event).await {
                    Ok(msg) => {
                        info!("Message {} published to message boker successfully", msg);
                    }
                    Err(e) => {
                        error!("Error publishing message to boker: {:?}", e);
                    }
                }
            }
            None | Some(true) => {
                // send message
                // This is a simplified example. In a real scenario, you might not have the DLQ handler here.
                // For now, we'll use a placeholder that just logs.
                let dlq_handler = |msg: EventPackage| async move {
                    error!(
                        "DLQ HANDLER (WEB): Message for event_id {} failed all retries.",
                        msg.get_event_id()
                    );
                };
                send_mesage_with_no_block(event, dlq_handler).await;
            }
        }
        ResponseBuilder::builder(false).ok()
    }

    /// cleanup inactive clients
    pub async fn cleanup_inactive_clients() {
        loop {
            //noblock thread
            tokio::time::sleep(CLEANUP_INTERVAL).await;
            let subs = CLIENT_SUBSCRIPTIONS.get().unwrap();
            let to_remove: Vec<String> = {
                let now = Instant::now();
                subs.iter()
                    .filter_map(|entry| {
                        let id = entry.key();
                        let meta = entry.value();
                        let last_activity = *meta.last_activity.lock().unwrap();
                        // channel no message and ttl timeout
                        if meta.sender.receiver_count() == 0 && now - last_activity > CHANNEL_TTL {
                            Some(id.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            };

            if !to_remove.is_empty() {
                let removed_count = to_remove.len();
                for id in to_remove {
                    subs.remove(&id);
                }
                if removed_count > 0 {
                    info!("Cleaned up {} inactive channels", removed_count);
                }
            }
        }
    }

    /// Get the payload for an event, if enable compression then return compressioned payload
    fn get_payload(message: &EventPackage) -> String {
        let mut headers: HashMap<String, String> = message.headers.clone().unwrap_or_default();
        headers.insert("send_time".to_string(), now_time_with_format(None));

        let org_payload = json!({
            "data": message.data.clone(),
            "headers": headers
        });

        // If the message has already been compressed, it need not be compressed again.
        let is_compressed_payload = match headers.get_key_value("message_compressed") {
            Some(value) => value.1 == "true",
            None => false,
        };

        let final_payload =
            if message_compression::is_compression_enabled() && !is_compressed_payload {
                match message_compression::compress_and_encode(&org_payload) {
                    Ok(compressed_payload) => compressed_payload,
                    Err(e) => {
                        warn!("Payload compression failed: {}. Sending uncompressed.", e);
                        org_payload
                    }
                }
            } else {
                org_payload
            };

        serde_json::to_string(&final_payload).unwrap_or_else(|e| {
            error!("Failed to serialize final payload: {}", e);
            // Fallback to a simple error JSON string
            r#"{"error":"payload serialization failed"}"#.to_string()
        })
    }

    /// Background task to retry sending a message.
    async fn retry_and_dispatch_message<F, Fut>(message: EventPackage, dlq_handler: Arc<F>)
    where
        F: Fn(EventPackage) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let retries = get_env_var("SEND_FAIL_RETRIES", Some("1")).unwrap_or(1);
        let delay_secs = get_env_var("SEND_FAIL_DELAY_SECONDS", Some("2")).unwrap_or(2);
        let retry_delay = Duration::from_secs(delay_secs);

        let payload_to_send = get_payload(&message);
        let payload_arc = Arc::new(payload_to_send);

        for attempt in 0..=retries {
            if attempt > 0 {
                warn!(
                    "Retrying to send message for event_id: {}, attempt {}/{}",
                    message.get_event_id(),
                    attempt,
                    retries
                );
                tokio::time::sleep(retry_delay).await;
            }

            let sse_sender = {
                let subs = CLIENT_SUBSCRIPTIONS.get().unwrap();
                subs.get(&message.get_event_id())
                    .map(|meta| meta.sender.clone())
            };

            if let Some(sender) = sse_sender {
                if sender.receiver_count() > 0 {
                    if sender.send(payload_arc.clone()).is_ok() {
                        info!(
                            "Message for event_id: {} sent successfully on retry attempt {}.",
                            message.get_event_id(),
                            attempt
                        );
                        return; // Success
                    }
                }
            }
        }

        // All retries failed.
        error!(
            "All retry attempts failed for event_id: {}. Handling as per configuration.",
            message.get_event_id()
        );

        let push_to_dlq =
            get_env_var("DEAD_MESSAGE_PUSH_DEAD_QUEUE", Some("false")).unwrap_or(false);

        if push_to_dlq {
            info!(
                "Pushing failed message for event_id: {} to dead-letter queue.",
                message.get_event_id()
            );
            dlq_handler(message).await;
        } else {
            warn!(
                "Discarding message for event_id: {} after all retries failed (DLQ disabled).",
                message.get_event_id()
            );
        }
    }
}
