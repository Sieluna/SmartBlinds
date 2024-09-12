use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use time::OffsetDateTime;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::configs::storage::Storage;
use crate::models::sensor_data::SensorData;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        status: DeviceStatus,
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

#[derive(Clone)]
pub struct CommandResult {
    pub command_id: String,
    pub device_id: String,
    pub success: bool,
    pub message: String,
    pub timestamp: OffsetDateTime,
}

#[derive(Clone)]
pub struct ConnectionService {
    device_states: Arc<RwLock<HashMap<String, DeviceState>>>,
    // Sender for each device ID to send messages to devices
    device_senders: Arc<RwLock<HashMap<String, broadcast::Sender<CloudMessage>>>>,
    // Sender for each sensor ID to broadcast sensor data
    sensor_channels: Arc<RwLock<HashMap<String, broadcast::Sender<SensorData>>>>,
    // Command result channels
    command_results: Arc<RwLock<HashMap<String, broadcast::Sender<CommandResult>>>>,
    // Storage service
    storage: Arc<Storage>,
}

impl ConnectionService {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            device_states: Arc::new(RwLock::new(HashMap::new())),
            device_senders: Arc::new(RwLock::new(HashMap::new())),
            sensor_channels: Arc::new(RwLock::new(HashMap::new())),
            command_results: Arc::new(RwLock::new(HashMap::new())),
            storage,
        }
    }

    /// Handle device WebSocket connection
    pub async fn handle_device_connection(
        &self,
        device_id: String,
        ws: WebSocket,
    ) {
        tracing::info!("Device {} connected", device_id);
        
        // Create device message channel
        let (tx, _) = broadcast::channel(100);
        {
            let mut senders = self.device_senders.write().await;
            senders.insert(device_id.clone(), tx);
        }
        
        // Update device state
        {
            let mut states = self.device_states.write().await;
            states.insert(device_id.clone(), DeviceState {
                status: DeviceStatus::Connected,
                last_heartbeat: OffsetDateTime::now_utc(),
                retry_count: 0,
            });
        }
        
        // Split WebSocket into sender and receiver
        let (mut ws_sender, mut ws_receiver) = ws.split();
        
        // Create forwarding task from channel to WebSocket
        let device_id_clone = device_id.clone();
        let device_senders = self.device_senders.clone();
        let forward_task = tokio::spawn(async move {
            let tx = {
                let senders = device_senders.read().await;
                senders.get(&device_id_clone).cloned().unwrap()
            };
            
            let mut rx = tx.subscribe();
            
            while let Ok(msg) = rx.recv().await {
                let json = serde_json::to_string(&msg).unwrap();
                if let Err(e) = ws_sender.send(Message::Text(json)).await {
                    tracing::error!("Failed to send message to device {}: {}", device_id_clone, e);
                    break;
                }
            }
        });
        
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
                                tracing::error!("Failed to parse message from device {}: {}", device_id, e);
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
        
        // Update device state
        {
            let mut states = self.device_states.write().await;
            if let Some(state) = states.get_mut(&device_id) {
                state.status = DeviceStatus::Disconnected;
            }
        }
        
        // Clean up resources
        {
            let mut senders = self.device_senders.write().await;
            senders.remove(&device_id);
        }
    }
    
    /// Handle messages from device
    async fn handle_device_message(&self, device_id: &str, msg: DeviceMessage) {
        match msg {
            DeviceMessage::SensorData { sensor_id, light, temperature, timestamp } => {
                // Process sensor data
                let sensor_data = SensorData {
                    id: 0, // Database will auto-assign ID
                    sensor_id: sensor_id.parse().unwrap_or(0),
                    light,
                    temperature,
                    time: timestamp,
                };
                
                // Save to database
                match sqlx::query_as::<_, SensorData>(
                    r#"
                    INSERT INTO sensor_data (sensor_id, light, temperature, time)
                    VALUES ($1, $2, $3, $4)
                    RETURNING *;
                    "#
                )
                .bind(&sensor_data.sensor_id)
                .bind(&sensor_data.light)
                .bind(&sensor_data.temperature)
                .bind(sensor_data.time)
                .fetch_one(self.storage.get_pool())
                .await {
                    Ok(saved_data) => {
                        // Broadcast sensor data
                        let sensor_id_str = sensor_id.to_string();
                        let sender = {
                            let mut channels = self.sensor_channels.write().await;
                            channels.entry(sensor_id_str.clone())
                                .or_insert_with(|| broadcast::channel(100).0)
                                .clone()
                        };
                        
                        let _ = sender.send(saved_data);
                        tracing::debug!("Saved data for sensor {}", sensor_id);
                    }
                    Err(e) => {
                        tracing::error!("Failed to save data for sensor {}: {}", sensor_id, e);
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
                tracing::debug!("Device {} heartbeat updated", device_id);
            }
            DeviceMessage::Status { status, timestamp, .. } => {
                // Update device status
                let mut states = self.device_states.write().await;
                if let Some(state) = states.get_mut(device_id) {
                    state.status = status;
                    state.last_heartbeat = timestamp;
                }
                tracing::debug!("Device {} status updated", device_id);
            }
            DeviceMessage::CommandResult { command_id, result, message, timestamp, .. } => {
                // Handle command execution result
                let command_result = CommandResult {
                    command_id: command_id.clone(),
                    device_id: device_id.to_string(),
                    success: result,
                    message,
                    timestamp,
                };
                
                // Send command result
                let sender = {
                    let results = self.command_results.read().await;
                    results.get(&command_id).cloned()
                };
                
                if let Some(sender) = sender {
                    let _ = sender.send(command_result);
                    tracing::debug!("Processed command {} result for device {}", command_id, device_id);
                }
            }
        }
    }
    
    /// Subscribe to sensor data
    pub async fn subscribe_sensor(&self, sensor_id: String) -> broadcast::Receiver<SensorData> {
        let sender = {
            let mut channels = self.sensor_channels.write().await;
            channels.entry(sensor_id)
                .or_insert_with(|| broadcast::channel(100).0)
                .clone()
        };
        
        sender.subscribe()
    }
    
    /// Send control command
    pub async fn send_command(
        &self,
        device_id: &str,
        command: &str,
        priority: u8,
    ) -> Result<CommandResult, Box<dyn std::error::Error + Send + Sync>> {
        // Generate command ID
        let command_id = Uuid::new_v4().to_string();
        
        // Create command result channel
        let (result_tx, mut result_rx) = broadcast::channel(1);
        {
            let mut results = self.command_results.write().await;
            results.insert(command_id.clone(), result_tx);
        }
        
        // Create command message
        let msg = CloudMessage::ControlCommand {
            command_id: command_id.clone(),
            device_id: device_id.to_string(),
            command: command.to_string(),
            priority,
            timestamp: OffsetDateTime::now_utc(),
        };
        
        // Send command
        let sender = {
            let senders = self.device_senders.read().await;
            senders.get(device_id).cloned()
        };
        
        if let Some(sender) = sender {
            // Send command
            sender.send(msg)?;
            
            // Wait for result with timeout
            match tokio::time::timeout(Duration::from_secs(5), result_rx.recv()).await {
                Ok(Ok(result)) => {
                    // Clean up command result channel
                    let mut results = self.command_results.write().await;
                    results.remove(&command_id);
                    
                    Ok(result)
                }
                _ => {
                    // Timeout or error
                    let mut results = self.command_results.write().await;
                    results.remove(&command_id);
                    
                    Err("Command timeout or device not responding".into())
                }
            }
        } else {
            Err("Device not connected".into())
        }
    }
    
    /// Get device status
    pub async fn get_device_status(&self, device_id: &str) -> Option<DeviceStatus> {
        let states = self.device_states.read().await;
        states.get(device_id).map(|state| state.status.clone())
    }
    
    /// Start heartbeat check
    pub fn start_heartbeat_check(&self) {
        const MAX_RETRIES: u32 = 3;
        let device_states = self.device_states.clone();
        let device_senders = self.device_senders.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // Send heartbeat request
                let msg = CloudMessage::HeartbeatRequest {
                    timestamp: OffsetDateTime::now_utc(),
                };
                
                // Get all device IDs
                let device_ids = {
                    let states = device_states.read().await;
                    states.keys().cloned().collect::<Vec<_>>()
                };
                
                for device_id in device_ids {
                    // Check device status
                    let mut is_disconnected = false;
                    {
                        let mut states = device_states.write().await;
                        if let Some(state) = states.get_mut(&device_id) {
                            let now = OffsetDateTime::now_utc();
                            let duration = now - state.last_heartbeat;
                            
                            if duration > time::Duration::hours(2) {
                                state.status = DeviceStatus::Disconnected;
                                state.retry_count += 1;
                                is_disconnected = true;
                                
                                if state.retry_count > MAX_RETRIES {
                                    // Remove device from active connections
                                    let mut senders = device_senders.write().await;
                                    senders.remove(&device_id);
                                    tracing::warn!("Device {} considered lost after {} retries", device_id, MAX_RETRIES);
                                }
                            }
                        }
                    }
                    
                    // Send heartbeat request if device is online
                    if !is_disconnected {
                        let sender = {
                            let senders = device_senders.read().await;
                            senders.get(&device_id).cloned()
                        };
                        
                        if let Some(sender) = sender {
                            let _ = sender.send(msg.clone());
                        }
                    }
                }
            }
        });
    }
}
