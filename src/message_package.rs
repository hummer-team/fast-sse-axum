
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPackage {
    pub data: Option<serde_json::Value>,
    pub headers: Option<HashMap<String, String>>,
    pub event_name: String,
    pub event_id: Option<String>,
    pub event_type: Option<String>,
    pub user: Option<User>,
    pub send_direct: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub from_user: String,
    pub to_user: String,
}

impl EventPackage {
    pub fn new(
        event_name: String,
        event_id: String,
        from_user: String,
        to_user: String,
        data: Option<serde_json::Value>,
        headers: Option<HashMap<String, String>>,
        event_type: Option<String>,
        send_direct: Option<bool>,
    ) -> Self {
        EventPackage {
            event_name: event_name.to_string(),
            event_id: Some(event_id.to_string()),
            user: Some(User {
                from_user: from_user.to_string(),
                to_user: to_user.to_string(),
            }),
            data: data,
            headers: headers,
            event_type: event_type,
            send_direct: send_direct,
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
        match self.event_id {
            Some(ref e) => e.to_string(),
            None => "NA".to_string(),
        }
    }

    pub fn get_event_type(&self) -> String {
        match self.event_type {
            Some(ref e) => e.to_string(),
            None => "NA".to_string(),
        }
    }
}
