#[cfg(test)]
mod config_loader_tests {
    use crate::common::config_loader::config_loader;
    use std::collections::HashMap;
    use std::env;
    use std::sync::Mutex;
    use toml::{Value, value::Datetime};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_flatten_and_set_vars_all_types() {
        let _lock = ENV_LOCK.lock().unwrap();

        let mut config = HashMap::new();
        config.insert(
            "STRING_KEY".to_string(),
            Value::String("hello world".to_string()),
        );
        config.insert("INT_KEY".to_string(), Value::Integer(123));
        config.insert("FLOAT_KEY".to_string(), Value::Float(45.6));
        config.insert("BOOL_KEY".to_string(), Value::Boolean(true));

        let datetime_str = "2025-01-01T12:00:00Z";
        let datetime: Datetime = datetime_str.parse().unwrap();
        config.insert("DATETIME_KEY".to_string(), Value::Datetime(datetime));

        let array_values = vec![
            Value::String("a".to_string()),
            Value::Integer(1),
            Value::Boolean(false),
        ];
        config.insert("ARRAY_KEY".to_string(), Value::Array(array_values));

        let mut nested_table = HashMap::new();
        nested_table.insert("HOST".to_string(), Value::String("localhost".to_string()));
        nested_table.insert("PORT".to_string(), Value::Integer(5432));

        let mut deeply_nested = HashMap::new();
        deeply_nested.insert("MAX_SIZE".to_string(), Value::Integer(20));
        nested_table.insert(
            "POOL".to_string(),
            Value::Table(deeply_nested.into_iter().collect()),
        );

        config.insert(
            "DATABASE".to_string(),
            Value::Table(nested_table.into_iter().collect()),
        );

        let keys_to_cleanup = [
            "STRING_KEY",
            "INT_KEY",
            "FLOAT_KEY",
            "BOOL_KEY",
            "DATETIME_KEY",
            "ARRAY_KEY",
            "DATABASE_HOST",
            "DATABASE_PORT",
            "DATABASE_POOL_MAX_SIZE",
        ];

        // invoke the function under test
        let result = config_loader::flatten_and_set_vars(&config, "");
        assert!(result.is_ok());

        // assert
        assert_eq!(env::var("STRING_KEY").unwrap(), "hello world");
        assert_eq!(env::var("INT_KEY").unwrap(), "123");
        assert_eq!(env::var("FLOAT_KEY").unwrap(), "45.6");
        assert_eq!(env::var("BOOL_KEY").unwrap(), "true");
        assert_eq!(env::var("DATETIME_KEY").unwrap(), datetime_str);
        assert_eq!(env::var("ARRAY_KEY").unwrap(), "a,1,false");
        assert_eq!(env::var("DATABASE_HOST").unwrap(), "localhost");
        assert_eq!(env::var("DATABASE_PORT").unwrap(), "5432");
        assert_eq!(env::var("DATABASE_POOL_MAX_SIZE").unwrap(), "20");

        // cleanup
        for key in &keys_to_cleanup {
            unsafe {
                env::remove_var(key);
            }
        }
    }

    #[test]
    fn test_flatten_array_with_unsupported_type_is_skipped() {
        let _lock = ENV_LOCK.lock().unwrap();

        let mut config = HashMap::new();
        let array_values = vec![
            Value::String("good_value".to_string()),
            Value::Table(HashMap::new().into_iter().collect()),
            Value::Integer(99),
        ];
        config.insert("MIXED_ARRAY".to_string(), Value::Array(array_values));

        let result = config_loader::flatten_and_set_vars(&config, "");
        assert!(result.is_ok());

        // Table
        assert_eq!(env::var("MIXED_ARRAY").unwrap(), "good_value,99");

        // clean up
        unsafe {
            env::remove_var("MIXED_ARRAY");
        }
    }

    #[test]
    fn test_load_config_with_substitution_and_defaults() {
        let _lock = ENV_LOCK.lock().unwrap();

        // Setup: create a dummy config file and set env vars
        let config_dir = "config";
        let env_name = "test-subst";
        let config_filename = format!("app.{}.toml", env_name);
        let config_path = std::path::Path::new(config_dir).join(&config_filename);

        std::fs::create_dir_all(config_dir).unwrap();
        let toml_content = r#"
            KEY_WITH_DEFAULT = "{NOT_SET_VAR:default_value}"
            KEY_WITH_ENV = "{SET_VAR:default_value}"
            KEY_NO_DEFAULT = "{NOT_SET_VAR_NO_DEFAULT}"
            KEY_EMPTY_DEFAULT = "{NOT_SET_VAR_EMPTY_DEFAULT:}"
            "#;
        std::fs::write(&config_path, toml_content).unwrap();

        // Set environment for load_config to pick up the test config
        unsafe {
            env::set_var("ENV", env_name);
        }
        unsafe {
            env::set_var("SET_VAR", "env_value");
        }
        unsafe {
            env::remove_var("NOT_SET_VAR");
        }
        unsafe {
            env::remove_var("NOT_SET_VAR_NO_DEFAULT");
        }
        unsafe {
            env::remove_var("NOT_SET_VAR_EMPTY_DEFAULT");
        }

        // Keys to clean up afterwards
        let keys_to_cleanup = [
            "KEY_WITH_DEFAULT",
            "KEY_WITH_ENV",
            "KEY_NO_DEFAULT",
            "KEY_EMPTY_DEFAULT",
            "SET_VAR",
        ];

        // Run the function
        let result = config_loader::load_config();
        assert!(result.is_ok());

        // Assertions: check if env vars are set correctly
        assert_eq!(env::var("KEY_WITH_DEFAULT").unwrap(), "default_value");
        assert_eq!(env::var("KEY_WITH_ENV").unwrap(), "env_value");
        assert_eq!(env::var("KEY_NO_DEFAULT").unwrap(), "");
        assert_eq!(env::var("KEY_EMPTY_DEFAULT").unwrap(), "");

        // Cleanup
        std::fs::remove_file(&config_path).unwrap();
        std::fs::remove_dir_all(config_dir).unwrap();
        for key in &keys_to_cleanup {
            unsafe {
                env::remove_var(key);
            }
        }
        unsafe {
            env::remove_var("ENV");
        }
    }

    #[test]
    fn test_flatten_and_set_vars_array_edge_cases() {
        let _lock = ENV_LOCK.lock().unwrap();

        let mut config = HashMap::new();

        // 1. Empty array
        config.insert("EMPTY_ARRAY".to_string(), Value::Array(vec![]));

        // 2. Array with only unsupported types
        let unsupported_array = vec![
            Value::Table(HashMap::new().into_iter().collect()),
            Value::Array(vec![]),
        ];
        config.insert(
            "UNSUPPORTED_ONLY_ARRAY".to_string(),
            Value::Array(unsupported_array),
        );

        // 3. Array with datetime values
        let datetime_str1 = "2025-01-01T12:00:00Z";
        let datetime1: Datetime = datetime_str1.parse().unwrap();
        let datetime_str2 = "2025-01-02T12:00:00Z";
        let datetime2: Datetime = datetime_str2.parse().unwrap();
        let datetime_array = vec![Value::Datetime(datetime1), Value::Datetime(datetime2)];
        config.insert("DATETIME_ARRAY".to_string(), Value::Array(datetime_array));

        // 4. Nested array
        let mut nested_table = HashMap::new();
        let ports_array = vec![Value::Integer(5432), Value::Integer(5433)];
        nested_table.insert("PORTS".to_string(), Value::Array(ports_array));
        config.insert(
            "DATABASE".to_string(),
            Value::Table(nested_table.into_iter().collect()),
        );

        let keys_to_cleanup = [
            "EMPTY_ARRAY",
            "UNSUPPORTED_ONLY_ARRAY",
            "DATETIME_ARRAY",
            "DATABASE_PORTS",
        ];

        // Invoke the function
        let result = config_loader::flatten_and_set_vars(&config, "");
        assert!(result.is_ok());

        // Assertions
        assert_eq!(env::var("EMPTY_ARRAY").unwrap(), "");
        assert_eq!(env::var("UNSUPPORTED_ONLY_ARRAY").unwrap(), "");
        assert_eq!(
            env::var("DATETIME_ARRAY").unwrap(),
            "2025-01-01T12:00:00Z,2025-01-02T12:00:00Z"
        );
        assert_eq!(env::var("DATABASE_PORTS").unwrap(), "5432,5433");

        // Cleanup
        for key in &keys_to_cleanup {
            unsafe {
                env::remove_var(key);
            }
        }
    }
}
