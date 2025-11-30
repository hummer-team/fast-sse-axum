use crate::message_package::EventPackage;
use crate::redis_stream;
use async_trait::async_trait;

#[async_trait]
pub trait EventPublisher {
    /// publish event to message boker(e.g. redis or kafka or ...)
    async fn publish_event(&self, event: EventPackage) -> Result<String, String>;
}

pub struct RedisEventPublisher;

impl RedisEventPublisher {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EventPublisher for RedisEventPublisher {
    async fn publish_event(&self, event: EventPackage) -> Result<String, String> {
        match redis_stream::publish_event(event).await {
            Ok(msg) => Ok(msg),
            Err(e) => Err(e.to_string()),
        }
    }
}
