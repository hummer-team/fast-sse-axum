pub mod message_package {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct EventPackage {
        pub data: Option<serde_json::Value>,
        pub headers: Option<HashMap<String, String>>,
        pub event_name: String,
        pub event_id: String,
        pub event_type: Option<String>,
        pub user: Option<User>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct User {
        pub from_user: String,
        pub to_user: String,
    }

    impl EventPackage {
        pub fn new(
            message_name: String,
            message_id: String,
            from_user: String,
            to_user: String,
            data: Option<serde_json::Value>,
            headers: Option<HashMap<String, String>>,
            event_type: Option<String>,
        ) -> Self {
            EventPackage {
                event_name: message_name.to_string(),
                event_id: message_id.to_string(),
                user: Some(User {
                    from_user: from_user.to_string(),
                    to_user: to_user.to_string(),
                }),
                data: data,
                headers: headers,
                event_type: event_type,
            }
        }

        pub fn get_to_user(&self) -> Option<String> {
            match self.user {
                Some(ref u) => Some(u.to_user.clone()),
                None => None,
            }
        }

        pub fn get_from_user(&self) -> Option<String> {
            match self.user {
                Some(ref u) => Some(u.from_user.clone()),
                None => None,
            }
        }

        pub fn get_event_name(&self) -> String {
            self.event_name.to_string()
        }

        pub fn get_event_id(&self) -> String {
            self.event_id.to_string()
        }
    }

    impl EventPackage {
        /// 将 MessagePackage 转换为 SSE 格式的字符串
        pub fn to_sse_format(&self) -> Result<String, serde_json::Error> {
            let mut sse_message = String::new();

            // 添加事件类型（对应 SSE 的 event 字段）
            if !self.event_name.is_empty() {
                sse_message.push_str(&format!("event: {}\n", self.event_name));
            }

            // 添加消息 ID（对应 SSE 的 id 字段）
            if !self.event_id.is_empty() {
                sse_message.push_str(&format!("id: {}\n", self.event_id));
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
