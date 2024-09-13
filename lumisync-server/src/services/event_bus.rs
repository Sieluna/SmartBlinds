use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::{broadcast, RwLock};

use crate::models::sensor_data::SensorData;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EventPayload {
    // Device Data
    SensorData(SensorData),
    DeviceStatus {
        device_id: String,
        status: String,
        timestamp: OffsetDateTime,
    },
    CommandResult {
        command_id: String,
        device_id: String,
        success: bool,
        message: String,
        timestamp: OffsetDateTime,
    },

    // Environment Data
    WeatherData {
        location: String,
        temperature: f32,
        humidity: f32,
        wind_speed: f32,
        solar_radiation: f32,
        timestamp: OffsetDateTime,
    },
    EnvironmentData {
        region_id: i32,
        indoor_temp: f32,
        indoor_light: i32,
        outdoor_temp: f32,
        timestamp: OffsetDateTime,
    },

    // Intelligent Control
    ControlRecommendation {
        region_id: i32,
        window_settings: Vec<WindowSetting>,
        reason: String,
        confidence: f32,
        timestamp: OffsetDateTime,
    },

    // User Interaction
    UserCommand {
        user_id: i32,
        command: String,
        target_id: String,
        timestamp: OffsetDateTime,
    },

    // Generic Event
    Generic {
        event_type: String,
        data: String,
        timestamp: OffsetDateTime,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowSetting {
    pub window_id: i32,
    pub state: f32,
    pub priority: u8,
}

pub struct EventBus {
    publishers: Arc<RwLock<HashMap<String, broadcast::Sender<EventPayload>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            publishers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn publish(
        &self,
        event_type: &str,
        payload: EventPayload,
    ) -> Result<usize, broadcast::error::SendError<EventPayload>> {
        let sender = {
            let mut publishers = self.publishers.write().await;
            publishers
                .entry(event_type.to_string())
                .or_insert_with(|| broadcast::channel(100).0)
                .clone()
        };

        sender.send(payload)
    }

    pub async fn subscribe(&self, event_type: &str) -> broadcast::Receiver<EventPayload> {
        let sender = {
            let mut publishers = self.publishers.write().await;
            publishers
                .entry(event_type.to_string())
                .or_insert_with(|| broadcast::channel(100).0)
                .clone()
        };

        sender.subscribe()
    }

    pub async fn has_subscribers(&self, event_type: &str) -> bool {
        let publishers = self.publishers.read().await;
        if let Some(sender) = publishers.get(event_type) {
            sender.receiver_count() > 0
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_publish_subscribe() {
        let event_bus = EventBus::new();

        let mut receiver1 = event_bus.subscribe("test.event").await;
        let mut receiver2 = event_bus.subscribe("test.event").await;

        let payload = EventPayload::Generic {
            event_type: "test".to_string(),
            data: "test data".to_string(),
            timestamp: OffsetDateTime::now_utc(),
        };

        let receiver_count = event_bus
            .publish("test.event", payload.clone())
            .await
            .unwrap();
        assert_eq!(receiver_count, 2);

        assert!(matches!(
            receiver1.recv().await,
            Ok(EventPayload::Generic { .. })
        ));
        assert!(matches!(
            receiver2.recv().await,
            Ok(EventPayload::Generic { .. })
        ));
    }

    #[tokio::test]
    async fn test_multiple_topics() {
        let event_bus = EventBus::new();

        let mut receiver1 = event_bus.subscribe("topic1").await;
        let mut receiver2 = event_bus.subscribe("topic2").await;

        let payload1 = EventPayload::Generic {
            event_type: "topic1".to_string(),
            data: "data1".to_string(),
            timestamp: OffsetDateTime::now_utc(),
        };

        event_bus.publish("topic1", payload1).await.unwrap();

        let payload2 = EventPayload::Generic {
            event_type: "topic2".to_string(),
            data: "data2".to_string(),
            timestamp: OffsetDateTime::now_utc(),
        };

        event_bus.publish("topic2", payload2).await.unwrap();

        assert!(matches!(
            receiver1.recv().await,
            Ok(EventPayload::Generic { .. })
        ));
        assert!(matches!(
            receiver2.recv().await,
            Ok(EventPayload::Generic { .. })
        ));
    }

    #[tokio::test]
    async fn test_has_subscribers() {
        let event_bus = EventBus::new();

        assert!(!event_bus.has_subscribers("test.event").await);

        let _receiver = event_bus.subscribe("test.event").await;

        assert!(event_bus.has_subscribers("test.event").await);
    }
}
