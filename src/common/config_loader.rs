pub mod config_loader {
    use regex::Regex;
    use std::collections::HashMap;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use toml::Value;
    use tracing::{error, info, warn};

    /// Loads configuration from a TOML file based on the `ENV` environment variable,
    /// performs environment variable substitution, and injects the settings into
    /// the current process's environment.
    pub fn load_config() -> Result<(), Box<dyn std::error::Error>> {
        // 1. Determine the environment (e.g., dev, uat, prd), defaulting to "dev".
        let env = if cfg!(debug_assertions) {
            env::var("ENV").unwrap_or_else(|_| "dev".to_string())
        } else {
            env::var("ENV").map_err(|e| {
                format!("The 'ENV' environment variable must be set in release mode (e.g., 'uat', 'prd'). Error: {}", e)
            })?
        };
        let config_filename = format!("app.{}.toml", env);
        let config_path = PathBuf::from("config").join(&config_filename);

        info!(
            "Attempting to load configuration for ENV='{}' from '{}'",
            env,
            config_path.display()
        );

        // 2. Read the configuration file.
        let content = match fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                let error_message = format!(
                    "Configuration file '{}' not found. This is a fatal error, process will terminate. Details: {}",
                    config_path.display(),
                    e
                );
                error!("{}", error_message);
                return Err(error_message.into());
            }
        };

        // 3. Perform environment variable substitution for "{VAR_NAME}" placeholders.
        let re = Regex::new(r"\{([A-Z0-9_]+)(?::([^}]*))?\}")?;
        let substituted_content = re.replace_all(&content, |caps: &regex::Captures| {
            let var_name = &caps[1];
            match env::var(var_name) {
                Ok(val) => val,
                Err(_) => match caps.get(2) {
                    Some(default_val) => default_val.as_str().to_string(),
                    None => {
                        warn!(
                            "Environment variable '{}' for substitution not found and no default value was provided.",
                            var_name
                        );
                        // Replace with empty string if not found
                        String::new() 
                    }
                },
            }
        });

        // 4. Parse the substituted TOML content.
        let config: HashMap<String, Value> = toml::from_str(&substituted_content)?;

        // 5. Flatten the configuration and inject it into the process environment.
        flatten_and_set_vars(&config, "")?;

        info!(
            "Successfully loaded and injected configuration from '{}'.",
            config_path.display()
        );

        Ok(())
    }

    /// Recursively flattens the TOML value and sets environment variables.
    /// e.g., `[redis.pool]` with `max_size = 10` becomes `REDIS_POOL_MAX_SIZE=10`.
    pub fn flatten_and_set_vars(
        config: &HashMap<String, Value>,
        prefix: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for (key, value) in config {
            let new_prefix = if prefix.is_empty() {
                key.to_uppercase()
            } else {
                format!("{}_{}", prefix, key.to_uppercase())
            };

            match value {
                Value::String(s) => unsafe { env::set_var(&new_prefix, s) },
                Value::Integer(i) => unsafe { env::set_var(&new_prefix, i.to_string()) },
                Value::Float(f) => unsafe { env::set_var(&new_prefix, f.to_string()) },
                Value::Boolean(b) => unsafe { env::set_var(&new_prefix, b.to_string()) },
                Value::Table(table) => {
                    // In TOML, tables are represented as HashMaps.
                    // We need to convert it to a HashMap<String, Value> to recurse.
                    let sub_map: HashMap<String, Value> = table.clone().into_iter().collect();
                    flatten_and_set_vars(&sub_map, &new_prefix)?;
                }
                Value::Datetime(dt) => unsafe { env::set_var(&new_prefix, dt.to_string()) },
                Value::Array(arr) => {
                    // Convert array of simple values to a comma-separated string.
                    // This is a common convention for environment variables.
                    let value_str = arr
                        .iter()
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            Value::Integer(i) => i.to_string(),
                            Value::Float(f) => f.to_string(),
                            Value::Boolean(b) => b.to_string(),
                            Value::Datetime(dt) => dt.to_string(),
                            _ => {
                                warn!(
                                    "Unsupported value type {:?} in array for key '{}'",
                                    v, new_prefix
                                );
                                String::new()
                            }
                        })
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(",");
                    unsafe { env::set_var(&new_prefix, value_str) }
                }
            }
        }
        Ok(())
    }
}
