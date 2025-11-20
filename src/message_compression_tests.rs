#[cfg(test)]
mod message_compression_tests {
    use crate::message_compression::message_compression;
    use base64::{engine::general_purpose, Engine as _};
    use flate2::read::GzDecoder;
    use serde_json::{json, Value};
    use std::io::Read;

    #[test]
    fn test_compress_and_encode_and_decode_roundtrip() {
        let original_data = json!({
            "user": "test",
            "message": "test message",
            "timestamp": 1678886400
        });

        let compressed_result = message_compression::compress_and_encode(&original_data);

        assert!(compressed_result.is_ok());
        let compressed_value = compressed_result.unwrap();

        let compressed_obj = compressed_value.as_object().unwrap();
        assert_eq!(
            compressed_obj.get("encoding").unwrap().as_str().unwrap(),
            "base64+gzip"
        );
        let encoded_data = compressed_obj.get("data").unwrap().as_str().unwrap();
        assert!(!encoded_data.is_empty());

        let compressed_bytes = general_purpose::STANDARD.decode(encoded_data).unwrap();

        let mut decoder = GzDecoder::new(&compressed_bytes[..]);
        let mut decompressed_json_string = String::new();
        decoder
            .read_to_string(&mut decompressed_json_string)
            .unwrap();

        let final_data: Value = serde_json::from_str(&decompressed_json_string).unwrap();

        assert_eq!(
            original_data, final_data,
            "往返测试失败：解压后的数据与原始数据不匹配"
        );
    }
}
