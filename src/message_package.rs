pub mod message_package {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessagePackage {
        pub data: Option<serde_json::Value>,
        pub headers: Option<HashMap<String, String>>,
        pub message_name: String,
        pub message_id: String,
        pub user: Option<User>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct User {
        pub from_user: String,
        pub to_user: String,
    }

    impl MessagePackage {
        pub fn new(
            message_name: &str,
            message_id: &str,
            from_user: &str,
            to_user: &str,
            data: Option<serde_json::Value>,
            headers: Option<HashMap<String, String>>,
        ) -> Self {
            MessagePackage {
                message_name: message_name.to_string(),
                message_id: message_id.to_string(),
                user: Some(User {
                    from_user: from_user.to_string(),
                    to_user: to_user.to_string(),
                }),
                data: data,
                headers: headers,
            }
        }

        pub fn get_to_user(&self) -> Option<&str> {
            self.user.as_ref().map(|u| u.to_user.as_str())
        }

        pub fn get_from_user(&self) -> Option<&str> {
            self.user.as_ref().map(|u| u.from_user.as_str())
        }

        pub fn get_event_name(&self) -> &str {
            &self.message_name
        }
    }

    impl MessagePackage {
        /// 将 MessagePackage 转换为 SSE 格式的字符串
        pub fn to_sse_format(&self) -> Result<String, serde_json::Error> {
            let mut sse_message = String::new();

            // 添加事件类型（对应 SSE 的 event 字段）
            if !self.message_name.is_empty() {
                sse_message.push_str(&format!("event: {}\n", self.message_name));
            }

            // 添加消息 ID（对应 SSE 的 id 字段）
            if !self.message_id.is_empty() {
                sse_message.push_str(&format!("id: {}\n", self.message_id));
            }

            // 序列化整个消息包作为数据内容
            let json_data = serde_json::to_string(self)?;
            sse_message.push_str(&format!("data: {}\n", json_data));

            // SSE 消息以空行结束
            sse_message.push_str("\n");

            Ok(sse_message)
        }
    }
}
