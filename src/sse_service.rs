pub mod sse_service {
    use crate::auth_middle::auth_middle::{auth, AuthConfig};
    use axum::extract::Path;
    use axum::middleware::from_fn_with_state;
    use axum::{
        response::sse::{Event, Sse},
        routing::get,
        Router,
    };
    use axum_extra::TypedHeader;
    use futures_util::stream::{self, Stream};
    use std::sync::Arc;
    use std::{
        collections::HashMap,
        sync::{OnceLock, RwLock},
    };
    use std::{convert::Infallible, path::PathBuf, time::Duration};
    use tokio::sync::broadcast;
    use tokio_stream::StreamExt as _;
    use tower_http::{services::ServeDir, trace::TraceLayer};
    use tracing::log::info;

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
                "/v1/sse/events/{event_id}/types/{event_type}",
                get(subscribert), // http method is get
            )
            .layer(TraceLayer::new_for_http())
            .layer(from_fn_with_state(config.clone(), auth));
        info!("sse router register success");
        router
    }

    // OnceLock 解决了静态变量的延迟初始化问题，因为rust要求静态变量必须常量
    // RwLock 读写锁，多线程读，单线程写
    static CLIENT_SUBSCRIPTIONS: OnceLock<RwLock<HashMap<String, broadcast::Sender<String>>>> =
        OnceLock::new();
    static SSE_BROAD_CAST: OnceLock<broadcast::Sender<String>> = OnceLock::new();

    /// 初始化函数
    ///
    /// # Examples
    ///
    /// ```
    /// use example_sse::sse_service::sse_service::init;
    ///
    /// assert_eq!(init(), );
    /// ```
    pub async fn init() {
        let (tx, _rx) = broadcast::channel(256);
        SSE_BROAD_CAST.get_or_init(|| tx);
        CLIENT_SUBSCRIPTIONS.get_or_init(|| RwLock::new(HashMap::new()));
        info!("SSE service subscript initialized");
    }

    /// Subscribe to an event
    pub async fn subscribert(Path((event_id, event_type)): Path<(String, String)>) {
        let subscriptions = CLIENT_SUBSCRIPTIONS.get().unwrap();
        let mut r = subscriptions.write().unwrap();
        if r.contains_key(&event_id) {
            return;
        }
        info!("Subscribed to event: {} {}", event_id, event_type);
        let tx = SSE_BROAD_CAST.get().unwrap().clone();
        r.insert(event_id, tx);
    }

    pub async fn sse_handler(
        Path(event_id): Path<String>,
        TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        println!("`{}` connected", user_agent.as_str());
        tracing::info!("`{}` sse event", event_id);
        // A `Stream` that repeats an event every second
        //
        // You can also create streams from tokio channels using the wrappers in
        // https://docs.rs/tokio-stream
        let stream = stream::repeat_with(|| Event::default().data("hi!"))
            .map(Ok)
            .throttle(Duration::from_secs(1));

        Sse::new(stream).keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(1))
                .text("keep-alive-text"),
        )
    }
}
