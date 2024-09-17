use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::{error, fs, io};

use rumqttc::tokio_rustls::rustls::{ClientConfig, RootCertStore};
use rumqttc::{
    AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS, TlsConfiguration, Transport,
};
use rustls_pemfile::{certs, read_one, Item};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::RwLock;

use crate::configs::settings::Gateway;
use crate::services::event_system::{EventBus, EventPayload};

/// MQTT data source configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MqttSourceConfig {
    pub name: String,
    pub topic_pattern: String,
    pub data_type: String,
    pub data_format: DataFormat,
    pub mapping: HashMap<String, String>,
}

/// Supported data formats
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum DataFormat {
    JSON,
    CSV,
    KeyValue,
    Binary,
    Custom(String),
}

/// Data processor trait
#[async_trait::async_trait]
pub trait DataProcessor: Send + Sync {
    /// Processor name
    fn name(&self) -> &str;

    /// Processor supported data formats
    fn supported_formats(&self) -> Vec<DataFormat>;

    /// Process data
    async fn process(
        &self,
        topic: &str,
        payload: &[u8],
        config: &ProcessorConfig,
    ) -> Result<EventPayload, Box<dyn error::Error + Send + Sync>>;
}

/// Processor configuration
#[derive(Clone, Debug)]
pub struct ProcessorConfig {
    pub data_format: DataFormat,
    pub mapping: HashMap<String, String>,
    pub target_event: String,
}

/// Enhanced sensor service
pub struct SensorService {
    client: Arc<RwLock<AsyncClient>>,
    event_loop: Arc<RwLock<EventLoop>>,
    event_bus: Arc<EventBus>,
    source_configs: Arc<RwLock<HashMap<String, MqttSourceConfig>>>,
    data_processors: Arc<RwLock<HashMap<String, Box<dyn DataProcessor>>>>,
}

impl SensorService {
    /// Create new enhanced sensor service
    pub async fn new(
        gateway: Gateway,
        event_bus: Arc<EventBus>,
    ) -> Result<Self, Box<dyn error::Error + Send + Sync>> {
        let mut options = MqttOptions::new(&gateway.client_id, &gateway.host, gateway.port);
        options.set_keep_alive(Duration::from_secs(5));

        if let Some(auth) = &gateway.auth {
            let mut root_cert_store = RootCertStore::empty();
            root_cert_store
                .add_parsable_certificates(rustls_native_certs::load_native_certs().unwrap());

            let certs = certs(&mut io::BufReader::new(fs::File::open(&auth.cert_path)?))
                .map(|result| result.unwrap())
                .collect();
            let mut key_buffer = io::BufReader::new(fs::File::open(&auth.key_path)?);
            let key = loop {
                match read_one(&mut key_buffer)? {
                    Some(Item::Sec1Key(key)) => break key.into(),
                    Some(Item::Pkcs1Key(key)) => break key.into(),
                    Some(Item::Pkcs8Key(key)) => break key.into(),
                    None => return Err("no keys found or encrypted keys not supported".into()),
                    _ => {}
                }
            };

            let tls_config = ClientConfig::builder()
                .with_root_certificates(root_cert_store)
                .with_client_auth_cert(certs, key)?;

            options.set_transport(Transport::Tls(TlsConfiguration::from(tls_config)));
        }

        let (client, event_loop) = AsyncClient::new(options, 10);

        Ok(Self {
            client: Arc::new(RwLock::new(client)),
            event_loop: Arc::new(RwLock::new(event_loop)),
            event_bus,
            source_configs: Arc::new(RwLock::new(HashMap::new())),
            data_processors: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Register data processor
    pub async fn register_processor(&self, processor: Box<dyn DataProcessor>) {
        let name = processor.name().to_string();
        let mut processors = self.data_processors.write().await;
        processors.insert(name, processor);
    }

    /// Add data source configuration
    pub async fn add_data_source(
        &self,
        config: MqttSourceConfig,
    ) -> Result<(), Box<dyn error::Error + Send + Sync>> {
        // Save configuration
        {
            let mut configs = self.source_configs.write().await;
            configs.insert(config.name.clone(), config.clone());
        }

        // Subscribe to topic
        self.subscribe(&config.topic_pattern).await?;

        Ok(())
    }

    /// Remove data source configuration
    pub async fn remove_data_source(
        &self,
        name: &str,
    ) -> Result<(), Box<dyn error::Error + Send + Sync>> {
        let topic_to_unsubscribe = {
            let mut configs = self.source_configs.write().await;
            configs.remove(name).map(|config| config.topic_pattern)
        };

        if let Some(topic) = topic_to_unsubscribe {
            let client = self.client.write().await;
            client.unsubscribe(&topic).await?;
        }

        Ok(())
    }

    /// Subscribe to MQTT topic
    pub async fn subscribe(&self, topic: &str) -> Result<(), Box<dyn error::Error + Send + Sync>> {
        let client = self.client.write().await;
        client.subscribe(topic, QoS::AtLeastOnce).await?;

        tracing::info!("Subscribed to topic: {}", topic);

        Ok(())
    }

    /// Start MQTT event loop
    pub async fn start(&self) -> Result<(), Box<dyn error::Error + Send + Sync>> {
        let event_loop = self.event_loop.clone();
        let source_configs = self.source_configs.clone();
        let data_processors = self.data_processors.clone();
        let event_bus = self.event_bus.clone();

        tokio::task::spawn_local(async move {
            loop {
                // Get MQTT event
                let event = {
                    let mut event_loop_guard = event_loop.write().await;
                    event_loop_guard.poll().await
                };

                match event {
                    Ok(Event::Incoming(Packet::Publish(publish))) => {
                        // Get topic and payload
                        let topic = publish.topic.clone();
                        let payload = publish.payload.clone();

                        // Find matching data source configuration
                        let matching_configs = {
                            let configs = source_configs.read().await;
                            configs
                                .iter()
                                .filter(|(_, config)| topic_matches(&topic, &config.topic_pattern))
                                .map(|(_, config)| config.clone())
                                .collect::<Vec<MqttSourceConfig>>()
                        };

                        // Process each matching configuration
                        for config in matching_configs {
                            // Find suitable processor
                            let processors = data_processors.read().await;
                            if let Some(processor) = processors
                                .values()
                                .find(|p| p.supported_formats().contains(&config.data_format))
                            {
                                // Create processor configuration
                                let processor_config = ProcessorConfig {
                                    data_format: config.data_format.clone(),
                                    mapping: config.mapping.clone(),
                                    target_event: format!("sensor.data.{}", config.name),
                                };

                                // Process data
                                match processor.process(&topic, &payload, &processor_config).await {
                                    Ok(event_payload) => {
                                        // Publish to event bus
                                        if let Err(e) = event_bus
                                            .publish(&processor_config.target_event, event_payload)
                                            .await
                                        {
                                            tracing::error!("Failed to publish event: {}", e);
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to process data: {}", e);
                                    }
                                }
                            } else {
                                tracing::warn!(
                                    "No processor found for format: {:?}",
                                    config.data_format
                                );
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("MQTT error: {}", e);
                        // Brief pause to avoid looping too fast in case of connection issues
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });

        Ok(())
    }
}

/// Check if topic matches pattern
fn topic_matches(topic: &str, pattern: &str) -> bool {
    if pattern.contains('#') {
        let prefix = pattern.trim_end_matches("/#");
        topic.starts_with(prefix)
    } else if pattern.contains('+') {
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let topic_parts: Vec<&str> = topic.split('/').collect();

        if pattern_parts.len() != topic_parts.len() {
            return false;
        }

        for (p, t) in pattern_parts.iter().zip(topic_parts.iter()) {
            if *p != "+" && *p != *t {
                return false;
            }
        }

        true
    } else {
        topic == pattern
    }
}

/// JSON data processor
pub struct JsonDataProcessor;

impl JsonDataProcessor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl DataProcessor for JsonDataProcessor {
    fn name(&self) -> &str {
        "json_processor"
    }

    fn supported_formats(&self) -> Vec<DataFormat> {
        vec![DataFormat::JSON]
    }

    async fn process(
        &self,
        _topic: &str,
        payload: &[u8],
        config: &ProcessorConfig,
    ) -> Result<EventPayload, Box<dyn error::Error + Send + Sync>> {
        // Parse JSON
        let payload_str = String::from_utf8(payload.to_vec())?;
        let json: serde_json::Value = serde_json::from_str(&payload_str)?;

        // Apply mapping
        let mut light = 0;
        let mut temperature = 0.0;
        let mut timestamp = OffsetDateTime::now_utc();
        let mut _sensor_id = String::new();

        for (target_field, source_path) in &config.mapping {
            let value = get_json_value(&json, source_path);

            match target_field.as_str() {
                "id" | "sensor_id" => {
                    if let Some(v) = value {
                        _sensor_id = v.as_str().unwrap_or_default().to_string();
                    }
                }
                "light" => {
                    if let Some(v) = value {
                        light = v.as_i64().unwrap_or_default() as i32;
                    }
                }
                "temperature" | "temp" => {
                    if let Some(v) = value {
                        temperature = v.as_f64().unwrap_or_default() as f32;
                    }
                }
                "timestamp" | "time" => {
                    if let Some(v) = value {
                        if let Some(ts) = v.as_i64() {
                            timestamp = OffsetDateTime::from_unix_timestamp(ts)
                                .unwrap_or(OffsetDateTime::now_utc());
                        }
                    }
                }
                _ => {}
            }
        }

        // Create environment data event
        let event = EventPayload::RegionData {
            region_id: 0, // Needs to be mapped to region
            indoor_temperature: temperature,
            indoor_light: light,
            outdoor_temperature: 0.0, // Needs to be fetched from other source
            timestamp,
        };

        Ok(event)
    }
}

/// Get value from JSON object by path
fn get_json_value<'a>(json: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = json;

    for part in parts {
        match current.get(part) {
            Some(value) => {
                current = value;
            }
            None => return None,
        }
    }

    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_matching() {
        // Exact match
        assert!(topic_matches("sensors/temp", "sensors/temp"));

        // Wildcard match
        assert!(topic_matches("sensors/temp", "sensors/#"));
        assert!(topic_matches("sensors/temp/room1", "sensors/#"));

        // Single level wildcard
        assert!(topic_matches("sensors/temp", "sensors/+"));
        assert!(topic_matches("sensors/humidity", "sensors/+"));
        assert!(!topic_matches("sensors/temp/room1", "sensors/+"));

        // Mixed wildcard
        assert!(topic_matches("home/sensors/temp", "home/sensors/+"));
        assert!(topic_matches("home/sensors/temp/room1", "home/sensors/#"));
        assert!(!topic_matches("office/sensors/temp", "home/sensors/#"));
    }

    #[tokio::test]
    async fn test_json_processor() {
        let processor = JsonDataProcessor::new();

        let json_payload = r#"{
            "device": {
                "id": "sensor123",
                "type": "environmental"
            },
            "readings": {
                "temperature": 25.5,
                "light": 800,
                "humidity": 60
            },
            "timestamp": 1609459200
        }"#;

        let mut mapping = HashMap::new();
        mapping.insert("sensor_id".to_string(), "device.id".to_string());
        mapping.insert(
            "temperature".to_string(),
            "readings.temperature".to_string(),
        );
        mapping.insert("light".to_string(), "readings.light".to_string());
        mapping.insert("timestamp".to_string(), "timestamp".to_string());

        let config = ProcessorConfig {
            data_format: DataFormat::JSON,
            mapping,
            target_event: "test.event".to_string(),
        };

        let result = processor
            .process("test/topic", json_payload.as_bytes(), &config)
            .await;

        assert!(result.is_ok());
        if let Ok(EventPayload::RegionData {
            indoor_temperature,
            indoor_light,
            timestamp,
            ..
        }) = result
        {
            assert_eq!(indoor_temperature, 25.5);
            assert_eq!(indoor_light, 800);
            assert_eq!(
                timestamp,
                OffsetDateTime::from_unix_timestamp(1609459200).unwrap()
            );
        } else {
            panic!("Processor returned wrong event type");
        }
    }
}
