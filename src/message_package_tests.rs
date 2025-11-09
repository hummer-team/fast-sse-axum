#[cfg(test)]
mod message_package_tests {
    use std::collections::HashMap;

    use crate::message_package::*;

    #[tokio::test]
    pub async fn message_package_tests() {
        let mut map = HashMap::new();
        map.insert("content-type", "application/json");

        let message_2 = message_package::MessagePackage::new(
            "message",
            "message_id",
            "alice",
            "bob",
            Some("Hello, World!"),
            Some(map),
        );

        assert_eq!(message_2.data.as_deref(), Some("Hello, World!"));
    }
}
