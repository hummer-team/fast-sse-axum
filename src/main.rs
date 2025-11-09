//! Run with
//!
//! ```not_rust
//! cargo run -p example-sse
//! ```
//! Test with
//! ```not_rust
//! cargo test -p example-sse
//! ```

use example_sse::log_wrapper::log_wrapper::init;
use example_sse::sse_service::sse_service::app;
use tracing::log::info;

#[tokio::main]
async fn main() {
    #[cfg(debug_assertions)]
    dotenvy::dotenv().ok();
    // Initialize logger
    let _guard = init();
    // build our application
    let app = app();
    // initialize sse service
    example_sse::sse_service::sse_service::init().await;
    info!("sse_service init done, starting server at 0.0.0.0:30000");
    // run it
    let listener = tokio::net::TcpListener::bind("0.0.0.0:30000")
        .await
        .unwrap();
    info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
