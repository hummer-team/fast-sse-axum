pub mod auth_middle {
    use crate::common::hmac_utils;
    use axum::{
        extract::{Request, State},
        http::StatusCode,
        middleware::Next,
        response::Response,
    };
    use std::{collections::HashSet, sync::Arc};
    use tracing::{debug, error, info};

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
        if path.eq("/") {
            return Ok(next.run(req).await);
        }

        // request path /v1/sse/events/{event_id}/types/{event_type}
        let segments: Vec<&str> = path.split('/').skip(1).collect();
        info!("request path: {}", path);
        let params = RquestParams {
            event_id: segments.get(3).ok_or(StatusCode::BAD_REQUEST)?.to_string(),
            event_type: segments.get(5).ok_or(StatusCode::BAD_REQUEST)?.to_string(),
        };
        if params.event_id.is_empty() || params.event_type.is_empty() {
            error!("Invalid event_id or event_type");
            return Err(StatusCode::BAD_REQUEST);
        }

        if config.allowed_event_ids.len() > 0 {
            if !config.allowed_event_ids.contains("*")
                && !config.allowed_event_ids.contains(&params.event_id)
            {
                error!("Event ID {} not allowed", params.event_id);
                return Err(StatusCode::FORBIDDEN);
            }
        }

        if config.allowed_event_types.len() > 0 {
            if !config.allowed_event_types.contains("*")
                && !config.allowed_event_types.contains(&params.event_type)
            {
                error!("Event type {} not allowed", params.event_type);
                return Err(StatusCode::FORBIDDEN);
            }
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
