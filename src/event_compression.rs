pub mod event_compression {
    use std::env;
    use tower_http::compression::{CompressionLayer, CompressionLevel};

    /// Create a compression layer
    #[warn(dead_code)]
    pub fn create_compression_layer() -> Option<CompressionLayer> {
        // read env var SEND_EVENT_TO_CLIENT_COMPRESSION
        let enabled = env::var("SEND_EVENT_TO_CLIENT_COMPRESSION")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        if enabled {
            Some(
                CompressionLayer::new()
                    .gzip(true)
                    .zstd(true)
                    .quality(CompressionLevel::Fastest),
            )
        } else {
            None
        }
    }
}
