#[cfg(test)]
mod sse_common_tests {
    use crate::common::sse_common::{ResourceResponse, now_time_with_format};

    #[test]
    pub fn test_resource() {
        let mut response = ResourceResponse::ok(Some("test"));
        assert_eq!(response.code, "ok");

        response = ResourceResponse::bad_request("test");
        assert_eq!(response.code, "bad_request");
    }

    #[test]
    pub fn test_time_format() {
        let time = now_time_with_format(None);
        println!("{}", time);
    }
}
