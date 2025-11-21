use example_sse::log_wrapper::log_wrapper::init;
use example_sse::redis_stream::redis_stream;
use example_sse::sse_service::sse_service;
use example_sse::tcp_configuration::tcp_configuration;
use std::net::SocketAddr;
use tracing::log::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    dotenvy::dotenv().ok();
    // Initialize logger
    let _guard = init();
    // initialize sse service
    sse_service::init().await;
    // initialize redis stream
    redis_stream::init().await?;
    info!("sse_service init done, starting server at 0.0.0.0:30000");
    // run it
    //let listener = tokio::net::TcpListener::bind("0.0.0.0:30000")
    //    .await
    //    .unwrap();
    let addr: SocketAddr = "0.0.0.0:30000".parse()?;
    let listener = tcp_configuration::create_tcp_listener(addr)?;
    info!("listening on {}", listener.local_addr().unwrap());
    // application
    let app = sse_service::init_router()?;
    axum::serve(listener, app).await.unwrap();
    Ok(())
}
