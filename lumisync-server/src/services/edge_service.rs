use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::configs::storage::Storage;
use crate::models::sensor_data::SensorData;
use crate::services::event_system::{EventBus, EventPayload};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeviceStatus {
    Connected,
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceState {
    pub status: DeviceStatus,
    pub last_heartbeat: OffsetDateTime,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceMessage {
    SensorData {
        sensor_id: String,
        light: i32,
        temperature: f32,
        timestamp: OffsetDateTime,
    },
    Heartbeat {
        device_id: String,
        timestamp: OffsetDateTime,
    },
    Status {
        device_id: String,
        status: String,
        timestamp: OffsetDateTime,
    },
    CommandResult {
        command_id: String,
        device_id: String,
        result: bool,
        message: String,
        timestamp: OffsetDateTime,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CloudMessage {
    ControlCommand {
        command_id: String,
        device_id: String,
        command: String,
        priority: u8,
        timestamp: OffsetDateTime,
    },
    AIGuidance {
        device_id: String,
        guidance: String,
        confidence: f32,
        timestamp: OffsetDateTime,
    },
    HeartbeatRequest {
        timestamp: OffsetDateTime,
    },
}

/// Command execution result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandResult {
    pub command_id: String,
    pub device_id: String,
    pub success: bool,
    pub message: String,
    pub timestamp: OffsetDateTime,
}

/// Edge device management service
pub struct EdgeDeviceService {
    device_states: Arc<RwLock<HashMap<String, DeviceState>>>,
    event_bus: Arc<EventBus>,
    storage: Arc<Storage>,
}

impl EdgeDeviceService {
    /// Create a new edge device service
    pub fn new(event_bus: Arc<EventBus>, storage: Arc<Storage>) -> Self {
        let service = Self {
            device_states: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            storage,
        };

        // Start heartbeat check
        service.start_heartbeat_check();

        service
    }

    /// Handle edge device WebSocket connection
    pub async fn handle_connection(&self, device_id: String, ws: WebSocket) {
        tracing::info!("Edge device {} connected", device_id);

        // Update device status
        {
            let mut states = self.device_states.write().await;
            states.insert(
                device_id.clone(),
                DeviceState {
                    status: DeviceStatus::Connected,
                    last_heartbeat: OffsetDateTime::now_utc(),
                    retry_count: 0,
                },
            );
        }

        // Publish device connection event through event bus
        let _ = self
            .event_bus
            .publish(
                "device.status",
                EventPayload::DeviceStatus {
                    device_id: device_id.clone(),
                    device_type: "edge_device".to_string(),
                    status: "connected".to_string(),
                    timestamp: OffsetDateTime::now_utc(),
                },
            )
            .await;

        // Split WebSocket into sender and receiver
        let (mut ws_sender, mut ws_receiver) = ws.split();

        // Subscribe to device command events
        let mut command_receiver = self
            .event_bus
            .subscribe(&format!("device.command.{}", device_id))
            .await;

        // Create forwarding task from event bus to WebSocket
        let forward_task = {
            let device_id = device_id.clone();
            tokio::spawn(async move {
                while let Ok(event) = command_receiver.recv().await {
                    match event {
                        EventPayload::UserCommand { command, .. } => {
                            // Convert to device command
                            let cloud_msg = CloudMessage::ControlCommand {
                                command_id: Uuid::new_v4().to_string(),
                                device_id: device_id.clone(),
                                command,
                                priority: 1,
                                timestamp: OffsetDateTime::now_utc(),
                            };

                            // Send to WebSocket
                            if let Ok(json) = serde_json::to_string(&cloud_msg) {
                                if let Err(e) = ws_sender.send(Message::Text(json)).await {
                                    tracing::error!(
                                        "Failed to send message to device {}: {}",
                                        device_id,
                                        e
                                    );
                                    break;
                                }
                            }
                        }
                        _ => continue,
                    }
                }
            })
        };

        // Handle messages received from WebSocket
        while let Some(result) = ws_receiver.next().await {
            match result {
                Ok(msg) => {
                    if let Message::Text(text) = msg {
                        match serde_json::from_str::<DeviceMessage>(&text) {
                            Ok(device_msg) => {
                                self.handle_device_message(&device_id, device_msg).await;
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to parse message from device {}: {}",
                                    device_id,
                                    e
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("WebSocket error: {}", e);
                    break;
                }
            }
        }

        // Handle disconnection
        tracing::info!("Device {} disconnected", device_id);
        forward_task.abort();

        // Update device status
        {
            let mut states = self.device_states.write().await;
            if let Some(state) = states.get_mut(&device_id) {
                state.status = DeviceStatus::Disconnected;
            }
        }

        // Publish device disconnection event through event bus
        let _ = self
            .event_bus
            .publish(
                "device.status",
                EventPayload::DeviceStatus {
                    device_id,
                    device_type: "edge_device".to_string(),
                    status: "disconnected".to_string(),
                    timestamp: OffsetDateTime::now_utc(),
                },
            )
            .await;
    }

    /// Handle device messages
    async fn handle_device_message(&self, device_id: &str, msg: DeviceMessage) {
        match msg {
            DeviceMessage::SensorData {
                sensor_id,
                light,
                temperature,
                timestamp,
            } => {
                // Save to database
                match sqlx::query(
                    r#"
                    INSERT INTO sensor_data (sensor_id, light, temperature, time)
                    VALUES ((SELECT id from sensors WHERE name = $1), $2, $3, $4)
                    "#,
                )
                .bind(&sensor_id)
                .bind(light)
                .bind(temperature)
                .bind(timestamp)
                .execute(self.storage.get_pool())
                .await
                {
                    Ok(_) => {
                        // Publish sensor data through event bus
                        match sqlx::query_as::<_, SensorData>(
                            r#"
                            SELECT * FROM sensor_data
                            WHERE sensor_id = (SELECT id from sensors WHERE name = $1)
                            ORDER BY time DESC LIMIT 1
                            "#,
                        )
                        .bind(&sensor_id)
                        .fetch_one(self.storage.get_pool())
                        .await
                        {
                            Ok(data) => {
                                let _ = self
                                    .event_bus
                                    .publish(
                                        &format!("sensor.data.{}", sensor_id),
                                        EventPayload::SensorData {
                                            sensor_id: sensor_id.parse::<i32>().unwrap_or(0),
                                            light: data.light,
                                            temperature: data.temperature,
                                            timestamp: data.time,
                                        },
                                    )
                                    .await;
                            }
                            Err(e) => tracing::error!("Failed to get sensor data: {}", e),
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to save data for sensor {}: {}", sensor_id, e)
                    }
                }
            }
            DeviceMessage::Heartbeat { timestamp, .. } => {
                // Update device heartbeat
                let mut states = self.device_states.write().await;
                if let Some(state) = states.get_mut(device_id) {
                    state.last_heartbeat = timestamp;
                    state.status = DeviceStatus::Connected;
                }
            }
            DeviceMessage::Status {
                status, timestamp, ..
            } => {
                // Update device status
                {
                    let mut states = self.device_states.write().await;
                    if let Some(state) = states.get_mut(device_id) {
                        state.last_heartbeat = timestamp;
                        state.status = match status.as_str() {
                            "connected" => DeviceStatus::Connected,
                            "disconnected" => DeviceStatus::Disconnected,
                            _ => DeviceStatus::Error(status.clone()),
                        };
                    }
                }

                // Publish device status event
                let _ = self
                    .event_bus
                    .publish(
                        "device.status",
                        EventPayload::DeviceStatus {
                            device_id: device_id.to_string(),
                            device_type: "edge_device".to_string(),
                            status,
                            timestamp,
                        },
                    )
                    .await;
            }
            DeviceMessage::CommandResult {
                command_id,
                result,
                message,
                timestamp,
                ..
            } => {
                // Publish command result event
                let _ = self
                    .event_bus
                    .publish(
                        &format!("command.result.{}", command_id),
                        EventPayload::CommandResult {
                            command_id,
                            device_id: device_id.to_string(),
                            success: result,
                            message,
                            timestamp,
                        },
                    )
                    .await;
            }
        }
    }

    /// Send command to device
    pub async fn send_command(
        &self,
        device_id: &str,
        command: &str,
        priority: u8,
    ) -> Result<CommandResult, Box<dyn std::error::Error + Send + Sync>> {
        // Check if device is online
        let is_device_online = {
            let states = self.device_states.read().await;
            matches!(
                states.get(device_id).map(|s| &s.status),
                Some(DeviceStatus::Connected)
            )
        };

        if !is_device_online {
            return Err("Device not connected".into());
        }

        // Generate command ID
        let command_id = Uuid::new_v4().to_string();

        // Subscribe to command result event
        let mut result_receiver = self
            .event_bus
            .subscribe(&format!("command.result.{}", command_id))
            .await;

        // Publish command event
        let _ = self
            .event_bus
            .publish(
                &format!("device.command.{}", device_id),
                EventPayload::UserCommand {
                    user_id: 0, // System command
                    command: command.to_string(),
                    timestamp: OffsetDateTime::now_utc(),
                },
            )
            .await;

        // Wait for command result
        match tokio::time::timeout(Duration::from_secs(5), result_receiver.recv()).await {
            Ok(Ok(EventPayload::CommandResult {
                command_id,
                device_id,
                success,
                message,
                timestamp,
            })) => Ok(CommandResult {
                command_id,
                device_id,
                success,
                message,
                timestamp,
            }),
            _ => Err("Command timed out or device did not respond".into()),
        }
    }

    /// Get device status
    pub async fn get_device_status(&self, device_id: &str) -> Option<DeviceStatus> {
        let states = self.device_states.read().await;
        states.get(device_id).map(|state| state.status.clone())
    }

    /// Get all device statuses
    pub async fn get_all_device_status(&self) -> HashMap<String, DeviceStatus> {
        let states = self.device_states.read().await;
        states
            .iter()
            .map(|(id, state)| (id.clone(), state.status.clone()))
            .collect()
    }

    /// Start heartbeat check
    fn start_heartbeat_check(&self) {
        const MAX_RETRIES: u32 = 3;
        let device_states = self.device_states.clone();
        let event_bus = self.event_bus.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                // Get all device IDs
                let device_ids = {
                    let states = device_states.read().await;
                    states.keys().cloned().collect::<Vec<_>>()
                };

                for device_id in device_ids {
                    // Check device status
                    let mut should_remove = false;
                    {
                        let mut states = device_states.write().await;
                        if let Some(state) = states.get_mut(&device_id) {
                            let now = OffsetDateTime::now_utc();
                            let duration = now - state.last_heartbeat;

                            if duration > time::Duration::hours(2) {
                                state.status = DeviceStatus::Disconnected;
                                state.retry_count += 1;

                                if state.retry_count > MAX_RETRIES {
                                    should_remove = true;
                                }
                            }
                        }
                    }

                    // Publish heartbeat request event
                    if !should_remove {
                        let heartbeat_payload = CloudMessage::HeartbeatRequest {
                            timestamp: OffsetDateTime::now_utc(),
                        };

                        if let Ok(json) = serde_json::to_string(&heartbeat_payload) {
                            let _ = event_bus
                                .publish(
                                    &format!("device.command.{}", device_id),
                                    EventPayload::Generic {
                                        event_type: "heartbeat_request".to_string(),
                                        data: json,
                                        timestamp: OffsetDateTime::now_utc(),
                                    },
                                )
                                .await;
                        }
                    } else {
                        // Remove device
                        let mut states = device_states.write().await;
                        states.remove(&device_id);

                        // Publish device removal event
                        let _ = event_bus
                            .publish(
                                "device.status",
                                EventPayload::DeviceStatus {
                                    device_id: device_id.clone(),
                                    device_type: "edge_device".to_string(),
                                    status: "removed".to_string(),
                                    timestamp: OffsetDateTime::now_utc(),
                                },
                            )
                            .await;

                        tracing::warn!(
                            "Device {} removed after {} retries",
                            device_id,
                            MAX_RETRIES
                        );
                    }
                }
            }
        });
    }
}
