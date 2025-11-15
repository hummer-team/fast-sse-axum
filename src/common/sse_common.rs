pub mod sse_common {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ResourceResponse<T> {
        pub code: String,
        pub message: Option<String>,
        pub data: Option<T>,
    }

    impl<T> ResourceResponse<T> {
        pub fn new(code: &str, message: &str, data: Option<T>) -> Self {
            Self {
                code: code.to_string(),
                message: Some(message.to_string()),
                data: data,
            }
        }

        pub fn ok(data: Option<T>) -> Self {
            Self {
                code: "ok".to_string(),
                message: Some("success".to_string()),
                data: data,
            }
        }

        pub fn error(code: &str, message: &str) -> Self {
            Self {
                code: code.to_string(),
                message: Some(message.to_string()),
                data: None,
            }
        }

        pub fn bad_request(message: &str) -> Self {
            Self::error("bad_request", message)
        }
    }
}
