pub mod auth_middle {
    use crate::common::hmac_utils;
    use axum::{
        extract::{Request, State},
        http::StatusCode,
        middleware::Next,
        response::Response,
    };
    use std::{collections::HashSet, sync::Arc};
    use tracing::{debug, error};

    #[derive(Clone)]
    pub struct AuthConfig {
        pub allowed_event_ids: HashSet<String>,
        pub allowed_event_types: HashSet<String>,
        pub hmac_secret: Vec<u8>,
    }

    impl AuthConfig {
        /// 构建 AuthConfig
        ///
        /// # 安全警告
        /// 生产环境必须从安全存储（如环境变量、密钥管理服务）加载密钥
        pub fn new(
            allowed_event_ids: HashSet<String>,
            hmac_secret: impl Into<Vec<u8>>,
            allowed_event_types: HashSet<String>,
        ) -> Self {
            AuthConfig {
                allowed_event_ids: allowed_event_ids,
                hmac_secret: hmac_secret.into(),
                allowed_event_types: allowed_event_types,
            }
        }
    }

    #[derive(Clone, Debug)]
    pub struct RquestParams {
        pub event_id: String,
        pub event_type: String,
    }

    /// Middleware
    /// for authenticating requests
    pub async fn auth(
        State(config): State<Arc<AuthConfig>>,
        mut req: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        let path = req.uri().path();
        // request path /v1/events/:event_id/types/:event_type
        let mut segments = path.split('/').skip(1);
        let params = RquestParams {
            event_id: segments.nth(2).ok_or(StatusCode::BAD_REQUEST)?.to_string(),
            event_type: segments.nth(4).ok_or(StatusCode::BAD_REQUEST)?.to_string(),
        };

        if params.event_id.is_empty() || params.event_type.is_empty() {
            debug!("Invalid event_id or event_type");
            return Err(StatusCode::BAD_REQUEST);
        }

        if config.allowed_event_ids.len() > 0
            && !config.allowed_event_ids.contains(&params.event_id)
        {
            debug!("Event ID not allowed");
            return Err(StatusCode::FORBIDDEN);
        }

        if config.allowed_event_types.len() > 0
            && !config.allowed_event_types.contains(&params.event_type)
        {
            debug!("Event type not allowed");
            return Err(StatusCode::FORBIDDEN);
        }

        let signature = req
            .headers()
            .get("x-signature")
            .and_then(|h| h.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        debug!("request x-signature is: {}", signature);

        let payload = format!("{}:{}", params.event_id, params.event_type);
        let verify = hmac_utils::hmac_utils::verify_sign(&config.hmac_secret, &payload, signature);
        if !verify {
            error!(
                "verify sign failed {} {}",
                params.event_id, params.event_type
            );
            return Err(StatusCode::UNAUTHORIZED);
        }

        req.extensions_mut().insert(RquestParams {
            event_id: params.event_id,
            event_type: params.event_type,
        });
        // continue processing
        Ok(next.run(req).await)
    }
}
