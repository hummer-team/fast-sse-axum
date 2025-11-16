#[cfg(test)]
mod redis_tests {
    use redis::RedisError;

    use crate::message_package::message_package;
    use crate::redis_pool::redis_pool;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init() {
        // init only once
        INIT.call_once(|| {
            // Initialize the evn here
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let env_file = manifest_dir.join(".test.env");
            println!("env_file: {:?}", env_file);
            dotenvy::from_filename(env_file).expect("env file load failed");
        });
    }

    #[tokio::test]
    pub async fn redis_set_test() {
        init();
        let pool = redis_pool::create_redis_pool().await;
        match redis_pool::get_conn(&pool).await {
            Ok(mut conn) => {
                let _: () = redis::cmd("SET")
                    .arg("test_key")
                    .arg("test_key")
                    .query_async(&mut *conn)
                    .await
                    .unwrap();
                let retult: String = redis::cmd("GET")
                    .arg("test_key")
                    .query_async(&mut *conn)
                    .await
                    .unwrap();
                assert_eq!("test_key", retult);
            }
            Err(e) => panic!("{}", e),
        };
    }

    #[tokio::test]
    pub async fn send_message_to_stream() {
        init();
        let pool = redis_pool::create_redis_pool().await;
        match redis_pool::get_conn(&pool).await {
            Ok(mut conn) => {
                let mut map = HashMap::new();
                map.insert("content-type".to_string(), "application/json".to_string());

                let msg = message_package::EventPackage::new(
                    "message".to_string(),
                    "message_id".to_string(),
                    "alice".to_string(),
                    "bob".to_string(),
                    serde_json::to_value("Hello, World!").ok(),
                    Some(map),
                    None,
                );
                let msg_json = serde_json::to_string(&msg).unwrap();

                let result: Result<String, RedisError> = redis::cmd("xadd")
                    .arg("test_stream")
                    .arg("*")
                    .arg("id:123")
                    .arg(&msg_json)
                    .query_async(&mut *conn)
                    .await;
                match result {
                    Ok(msg_id) => println!("xadd success message id: {}", msg_id),
                    Err(e) => println!("xadd fail: {}", e),
                }
                // subscribe step
                // XGROUP CREATE test_stream consumer_group 0 MKSTREAM
                // XREADGROUP GROUP consumer_group instance-1 COUNT 10 BLOCK 10000 STREAMS test_stream >
            }
            Err(e) => panic!("{}", e),
        };
    }
}
