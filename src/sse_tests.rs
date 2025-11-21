#[cfg(test)]
mod tests {
    use crate::sse_service::sse_service::init_router;
    use eventsource_stream::Eventsource;
    use tokio::net::TcpListener;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn integration_test() {
        // A helper function that spawns our application in the background
        async fn spawn_app(host: impl Into<String>) -> String {
            let host = host.into();
            // Bind to localhost at the port 0, which will let the OS assign an available port to us
            let listener = TcpListener::bind(format!("{host}:0")).await.unwrap();
            // Retrieve the port assigned to us by the OS
            let port = listener.local_addr().unwrap().port();
            tokio::spawn(async {
                axum::serve(listener, init_router().unwrap()).await.unwrap();
            });
            // Returns address (e.g. http://127.0.0.1{random_port})
            format!("http://{host}:{port}")
        }
        let listening_url = spawn_app("127.0.0.1").await;

        let mut event_stream = reqwest::Client::new()
            .get(format!("{listening_url}/sse"))
            .header("User-Agent", "integration_test")
            .send()
            .await
            .unwrap()
            .bytes_stream()
            .eventsource()
            .take(1);

        let mut event_data: Vec<String> = vec![];
        while let Some(event) = event_stream.next().await {
            match event {
                Ok(event) => {
                    // break the loop at the end of SSE stream
                    if event.data == "[DONE]" {
                        break;
                    }

                    event_data.push(event.data);
                }
                Err(_) => {
                    panic!("Error in event stream");
                }
            }
        }

        assert!(event_data[0] == "hi!");
    }

    #[test]
    pub fn path_params() {
        let params: Vec<&str> = "/v1/sse/events/carid000001/types/cart_changed"
            .split('/')
            .skip(1)
            .collect();
        println!("{:?}", params.len());
        assert_eq!(params.get(3), Some(&"carid000001"));
        assert_eq!(params.get(5), Some(&"cart_changed"));
    }

    #[test]
    pub fn string() {
        let s = "hello world";
        println!("{}", serde_json::Value::String(s.to_string()));
        println!("{}", serde_json::json!(s.to_string()));
    }
}
