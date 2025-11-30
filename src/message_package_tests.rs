#[cfg(test)]
mod message_package_tests {
    use std::collections::HashMap;

    use crate::message_package;

    #[tokio::test]
    pub async fn message_package_tests() {
        let mut map = HashMap::new();
        map.insert("content-type".to_string(), "application/json".to_string());

        let message_2 = message_package::EventPackage::new(
            "message".to_string(),
            "message_id".to_string(),
            "alice".to_string(),
            "bob".to_string(),
            serde_json::to_value("Hello, World!").ok(),
            Some(map),
            None,
            None,
        );
        println!("message_2 {:?}", message_2.data.as_ref().unwrap());
        assert_eq!(message_2.data, serde_json::to_value("Hello, World!").ok());
    }
}
