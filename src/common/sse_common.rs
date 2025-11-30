use chrono::Local;
use serde::{Deserialize, Serialize};
use std::env;
use std::str::FromStr;

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceResponse<T> {
    pub code: String,
    pub message: Option<String>,
    pub data: Option<T>,
}

impl<T> ResourceResponse<T> {
    pub fn new(code: &str, message: &str, data: Option<T>) -> Self {
        Self {
            code: code.to_string(),
            message: Some(message.to_string()),
            data: data,
        }
    }

    pub fn ok(data: Option<T>) -> Self {
        Self {
            code: "ok".to_string(),
            message: Some("success".to_string()),
            data: data,
        }
    }

    pub fn error(code: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            message: Some(message.to_string()),
            data: None,
        }
    }

    pub fn bad_request(message: &str) -> Self {
        Self::error("bad_request", message)
    }
}

/// Get an environment variable, returning an error if it is not set
pub fn get_env_var<T: FromStr>(name: &str, default: Option<&str>) -> Result<T, String> {
    let value_str = match env::var(name) {
        Ok(val) => val,
        Err(_) => match default {
            Some(d) => d.to_string(),
            None => {
                return Err(format!(
                    "Environment variable '{}' not set and no default value provided",
                    name
                ));
            }
        },
    };
    value_str.parse::<T>().map_err(|_| {
        format!(
            "Failed to parse env var '{}' with value '{}'",
            name, value_str
        )
    })
}

pub fn now_time_with_format(format: Option<&str>) -> String {
    let now = Local::now();
    match format {
        Some(format) => now.format(format).to_string(),
        None => now.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
    }
}
