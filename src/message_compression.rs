pub mod message_compression {
    use axum::response::sse::Event;
    use base64::{engine::general_purpose, Engine as _};
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use serde_json::{json, Value};
    use std::env;
    use std::io::Write;
    use tower_http::compression::{CompressionLayer, CompressionLevel};
    use tracing::{debug, warn};

    /// Create a compression layer
    #[warn(dead_code)]
    pub fn compression_layer_nosupport_sse() -> Option<CompressionLayer> {
        // read env var SEND_EVENT_TO_CLIENT_COMPRESSION
        let enabled = is_compression_enabled();

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

    /// process and compress an SSE event
    pub fn process_and_compress_event(json_value: Value) -> Event {
        if !is_compression_enabled() {
            return Event::default().json_data(&json_value).unwrap_or_else(|e| {
                warn!("sse event serialization error: {}", e);
                Event::default().data("serialization error")
            });
        }

        match compress_and_encode(&json_value) {
            Ok(compressed_payload) => Event::default()
                .json_data(&compressed_payload)
                .unwrap_or_else(|e| {
                    warn!("sse event compression error: {}", e);
                    Event::default().data("compression/serialization error")
                }),
            Err(e) => {
                warn!("server compression failed: {}", e);
                Event::default()
                    .event("error")
                    .data("server-side compression failed")
            }
        }
    }

    /// compression enabled
    pub fn is_compression_enabled() -> bool {
        env::var("SEND_EVENT_TO_CLIENT_COMPRESSION")
            .map(|val| val.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    fn compress_and_encode(json_value: &Value) -> Result<Value, Box<dyn std::error::Error>> {
        let data_bytes = serde_json::to_vec(json_value)?;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&data_bytes)?;
        let compressed_bytes = encoder.finish()?;
        debug!(
            "compressed {} bytes from {} bytes",
            data_bytes.len(),
            compressed_bytes.len()
        );
        let encoded_data = general_purpose::STANDARD.encode(&compressed_bytes);

        // must be a json object with "encoding" and "data" fields
        Ok(json!({ "encoding": "base64+gzip", "data": encoded_data }))
    }
}
