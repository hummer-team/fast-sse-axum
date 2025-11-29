use fast_sse::common::config_loader;
use fast_sse::log_wrapper::log_wrapper::init;
use fast_sse::redis_stream::redis_stream;
use fast_sse::sse_service::sse_service;
use fast_sse::tcp_configuration::tcp_configuration;
use std::net::SocketAddr;
use tracing::log::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    let _guard = init();
    // load config
    config_loader::config_loader::load_config()?;
    // initialize sse service
    sse_service::init().await;
    // initialize redis stream
    redis_stream::init().await?;
    info!("sse_service init done, starting server at 0.0.0.0:30000");
    //run it
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
