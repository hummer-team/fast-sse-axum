pub mod hmac_utils {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    pub fn verify_sign(secret: &[u8], payload: &str, provided_signature: &str) -> bool {
        let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC key must be valid");

        mac.update(payload.as_bytes());
        let result = mac.finalize();
        let expected_signature = hex::encode(result.into_bytes());
        //verify
        expected_signature.len() == provided_signature.len()
            && expected_signature
                .bytes()
                .zip(provided_signature.bytes())
                .fold(0, |acc, (a, b)| acc | (a ^ b))
                == 0
    }

    pub fn create_sign(secret: &[u8], payload: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC key must be valid");

        mac.update(payload.as_bytes());
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }
}
