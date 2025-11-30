#[cfg(test)]
mod hmac_utils_tests {
    use crate::common::hmac_utils::{create_sign, verify_sign};
    use std::path::PathBuf;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init() {
        INIT.call_once(|| {
            // Initialize the library here
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let env_file = manifest_dir.join(".test.env");
            println!("env_file: {:?}", env_file);
            dotenvy::from_filename(env_file).expect("env file load failed");
        });
    }

    #[test]
    fn test_create_and_verify_success() {
        init();
        let secret = std::env::var("HMAC_SECRET")
            .expect("HMAC_SECRET must be set")
            .into_bytes();

        // let secret = b"my_test_secret_key_32bytes_long!";
        let payload = "carid000001:cart_changed";

        let signature = create_sign(&secret, payload);
        // f8940179393d3be15724696499ba278eabd56874456b09349591640c41c6c35f
        println!("Generated signature: {}", signature);

        assert!(verify_sign(&secret, payload, &signature));
    }
}
