pub mod response_builder {
    use crate::common::sse_common::sse_common::ResourceResponse;
    use axum::response::IntoResponse;
    use axum::{
        body::Body,
        http::{header, Response, StatusCode},
        response::Json,
    };
    use std::convert::Infallible;

    pub struct ResponseBuilder {
        close: bool,
    }

    impl ResponseBuilder {
        pub fn builder(close: bool) -> Self {
            ResponseBuilder { close: close }
        }

        #[inline]
        pub fn forbidden<T>(self) -> Response<Body>
        where
            T: serde::Serialize,
        {
            let error_response = ResourceResponse::<()>::error("403", "No active subscription");
            let close_str: String = self.get_cnn_status_value();

            Response::builder()
                .status(StatusCode::FORBIDDEN)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::CONNECTION, close_str)
                .body(Json(error_response).into_response().into_body())
                .unwrap()
        }

        #[inline]
        pub fn ok(self) -> Response<Body> {
            let close_str: String = self.get_cnn_status_value();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONNECTION, close_str)
                .header(header::CONTENT_LENGTH, "0")
                .body(Body::empty())
                .unwrap()
        }

        #[inline]
        pub fn error(self, status: StatusCode, code: &str, message: &str) -> Response<Body> {
            let error_response = ResourceResponse::<()>::error(code, message);
            let close_str = self.get_cnn_status_value();
            Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::CONNECTION, close_str)
                .body(Json(error_response).into_response().into_body())
                .unwrap()
        }

        fn get_cnn_status_value(self) -> String {
            let close_str = if self.close {
                "close".to_string()
            } else {
                "keep-alive".to_string()
            };
            close_str
        }
    }
}
