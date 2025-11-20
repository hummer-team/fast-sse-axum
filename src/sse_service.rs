pub mod sse_service {
    use crate::auth_middle::auth_middle::{auth, AuthConfig};
    use crate::message_compression::message_compression::process_and_compress_event;
    use crate::message_package::message_package::EventPackage;
    use crate::response_builder::response_builder::ResponseBuilder;
    use axum::extract::Path;
    use axum::middleware::from_fn_with_state;
    use axum::{
        body::Body,
        http::StatusCode,
        response::{
            sse::{Event, KeepAlive, Sse},
            IntoResponse, Response,
        },
        routing::{get, post},
        Json, Router,
    };
    // use dashmap::DashMap;
    // use hashbrown::HashMap;
    use serde_json::Value;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::{collections::HashMap, sync::OnceLock};
    use std::{
        convert::Infallible,
        path::PathBuf,
        time::{Duration, Instant},
    };
    use tokio::sync::broadcast;
    use tokio::sync::RwLock;
    use tower_http::{services::ServeDir, trace::TraceLayer};
    use tracing::log::{debug, error, info, warn};

    /// SSE service enter
    pub fn app() -> Router {
        let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
        info!("Serving static files from {}", assets_dir.display());
        let static_files_service = ServeDir::new(assets_dir).append_index_html_on_directories(true);

        let allowed_ids = std::env::var("ALLOWED_EVENT_IDS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        let allowed_events = std::env::var("ALLOWED_EVENT_TYPES")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        let secret = std::env::var("HMAC_SECRET")
            .expect("HMAC_SECRET must be set")
            .into_bytes();

        let config = Arc::new(AuthConfig::new(allowed_ids, secret, allowed_events));

        // build our application with a route
        let router = Router::new()
            .fallback_service(static_files_service)
            .route(
                "/v1/subscribe/events/{event_id}/types/{event_type}",
                get(subscribert), // http method is get
            )
            .route(
                "/v1/sse/events/{event_id}/types/{event_type}",
                post(send_message_with_web_request), // http method is post
            )
            .layer(from_fn_with_state(config.into(), auth))
            .layer(TraceLayer::new_for_http());
        info!("sse router register success");
        router
    }

    struct ChannelMeta {
        sender: broadcast::Sender<Value>,
        last_activity: Mutex<Instant>,
    }

    /// 一个 event_id 对应一个 channel，所有订阅该 event_id 的客户端共享该 channel
    static CLIENT_SUBSCRIPTIONS: OnceLock<Arc<RwLock<HashMap<String, ChannelMeta>>>> =
        OnceLock::new();

    const CLEANUP_INTERVAL: Duration = Duration::from_secs(45);
    const CHANNEL_TTL: Duration = Duration::from_secs(120);
    /// init
    pub async fn init() {
        let _ = CLIENT_SUBSCRIPTIONS.set(Arc::new(RwLock::new(HashMap::new())));
        info!("SSE service subscript initialized");
        tokio::spawn(cleanup_inactive_clients());
    }

    /// Subscribe to an event
    pub async fn subscribert(Path((event_id, event_type)): Path<(String, String)>) -> Response {
        let receiver = {
            let mut subs = CLIENT_SUBSCRIPTIONS.get().unwrap().write().await;
            let meta = subs.entry(event_id.clone()).or_insert_with(|| {
                let (tx, _rx) = broadcast::channel::<Value>(1024);
                info!("create channel: {} (type: {})", event_id, event_type);
                ChannelMeta {
                    sender: tx,
                    last_activity: Mutex::new(Instant::now()),
                }
            });
            meta.sender.subscribe()
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
                        // push message to client
                        // yield Ok::<_, Infallible>(
                        //     Event::default()
                        //         .json_data(json_value)
                        //         .unwrap_or_else(|_| Event::default().data("serialization error"))
                        // );
                        let event = process_and_compress_event(json_value);
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

    pub async fn send_message(message: EventPackage) -> Result<bool, Option<(String, String)>> {
        let sse_sender = {
            let subs = CLIENT_SUBSCRIPTIONS.get().unwrap().read().await;
            if let Some(meta) = subs.get(&message.get_event_id()) {
                // * 表示解引用
                *meta.last_activity.lock().unwrap() = Instant::now();
                Some(meta.sender.clone())
            } else {
                None
            }
        }; // free the lock

        let Some(sender) = sse_sender else {
            warn!(
                "event_id = {},event_type = {},No active SSE subscription; messages discarded.",
                message.get_event_id(),
                message.event_name
            );
            let msg = (
                "NO_ACTIVE_SUBSCRIPTION".to_string(),
                "No active SSE subscription; messages discarded.".to_string(),
            );
            return Result::Err(Some(msg));
        };

        let event_id = message.get_event_id();
        let event_name = message.get_event_name();
        let event_type = message.get_event_type();

        match sender.send(message.data.unwrap()) {
            Ok(receivers_count) => {
                info!(
                    "event_id = {},event_type = {},receivers = {},Broadcast successful",
                    event_id, event_name, receivers_count
                );
                Result::Ok(true)
            }
            Err(broadcast::error::SendError(dropped)) => {
                error!(
                    "event_id = {},event_type = {},Broadcast queue full {}",
                    event_id, event_type, dropped
                );
                let msg = (
                    "SEND_FAIL".to_string(),
                    "No active SSE subscription; messages discarded.".to_string(),
                );
                Result::Err(Some(msg))
            }
        }
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
        );
        debug!(
            "sending message to client message id: {} message type: {}",
            event.get_event_id(),
            event.get_event_type()
        );
        // send message
        match send_message(event).await {
            // ignore ok responses values
            Ok(..) => ResponseBuilder::builder(false).ok(),
            Err(error) => ResponseBuilder::builder(false).error(
                StatusCode::OK,
                "Fail",
                error.unwrap().1.as_str(),
            ),
        }
    }

    /// cleanup inactive clients
    pub async fn cleanup_inactive_clients() {
        loop {
            //noblock thread
            tokio::time::sleep(CLEANUP_INTERVAL).await;
            let to_remove: Vec<String> = {
                let r = CLIENT_SUBSCRIPTIONS.get().unwrap().read().await;
                let now = Instant::now();
                r.iter()
                    .filter_map(|(id, meta)| {
                        let last_activity = *meta.last_activity.lock().unwrap();
                        // channel no message or ttl timeout
                        if meta.sender.receiver_count() == 0 || now - last_activity > CHANNEL_TTL {
                            Some(id.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            };

            let removed_count = if !to_remove.is_empty() {
                let mut w = CLIENT_SUBSCRIPTIONS.get().unwrap().write().await;
                let before = w.len();
                for id in to_remove {
                    w.remove(&id);
                }
                before - w.len()
            } else {
                0
            };

            if removed_count > 0 {
                info!("Cleaned up {} inactive channels", removed_count);
            } else {
                info!("No inactive channels to clean");
            }
        }
    }
}
