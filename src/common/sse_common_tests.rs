#[cfg(test)]
mod sse_common_tests {
    use crate::common::sse_common::sse_common::ResourceResponse;

    #[test]
    pub fn test_resource() {
        let mut response = ResourceResponse::ok(Some("test"));
        assert_eq!(response.code, "ok");

        response = ResourceResponse::bad_request("test");
        assert_eq!(response.code, "bad_request");
    }
}
