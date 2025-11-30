#[cfg(test)]
pub mod log_tests {
    use crate::log_wrapper::init;
    use tracing::log::{error, info, warn};

    #[test]
    fn test_log() {
        let _guard = init();
        info!("Hello, world!");
        error!("Hello, world!");
        warn!("Hello, world!");
    }
}
