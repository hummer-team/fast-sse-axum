pub mod message_package {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessagePackage<'a, T> {
        pub data: Option<T>,
        pub headers: Option<HashMap<&'a str, &'a str>>,
        pub message_name: &'a str,
        pub message_id: &'a str,
        pub user: Option<User<'a>>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct User<'a> {
        pub from_user: &'a str,
        pub to_user: &'a str,
    }

    impl<'a, T> MessagePackage<'a, T> {
        pub fn new(
            message_name: &'a str,
            message_id: &'a str,
            from_user: &'a str,
            to_user: &'a str,
            data: Option<T>,
            headers: Option<HashMap<&'a str, &'a str>>,
        ) -> Self {
            MessagePackage {
                message_name: message_name,
                message_id: message_id,
                user: Some(User {
                    from_user: from_user,
                    to_user: to_user,
                }),
                data,
                headers,
            }
        }

        pub fn get_to_user(&self) -> Option<&str> {
            self.user.as_ref().map(|u| u.to_user)
        }

        pub fn get_from_user(&self) -> Option<&str> {
            self.user.as_ref().map(|u| u.from_user)
        }

        pub fn get_event_name(&self) -> &str {
            &self.message_name
        }
    }

    impl<'a, T> MessagePackage<'a, T>
    where
        T: Serialize,
    {
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
