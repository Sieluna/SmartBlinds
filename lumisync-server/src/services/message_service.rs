use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;

use lumisync_api::message::*;
use lumisync_api::{DeviceStatus, DeviceValue, Id};
use time::OffsetDateTime;
use tokio::sync::{broadcast, oneshot};
use uuid::Uuid;

use crate::configs::Storage;
use crate::services::protocol::*;

#[derive(Debug, Clone)]
pub struct MessageServiceConfig {
    pub websocket_addr: SocketAddr,
    pub tcp_addr: SocketAddr,
    pub enable_websocket: bool,
    pub enable_tcp: bool,
}

impl Default for MessageServiceConfig {
    fn default() -> Self {
        Self {
            websocket_addr: "127.0.0.1:8080".parse().unwrap(),
            tcp_addr: "127.0.0.1:9000".parse().unwrap(),
            enable_websocket: true,
            enable_tcp: true,
        }
    }
}

pub struct MessageService {
    storage: Arc<Storage>,
    protocol_manager: Arc<ProtocolManager>,
    client_tx: broadcast::Sender<Message>,
    edge_tx: broadcast::Sender<Message>,
    stop_tx: Option<oneshot::Sender<()>>,
}

impl MessageService {
    pub fn new(storage: Arc<Storage>) -> Self {
        let (client_tx, _) = broadcast::channel(100);
        let (edge_tx, _) = broadcast::channel(100);

        let protocol_manager = ProtocolManager::new();

        Self {
            storage,
            protocol_manager: Arc::new(protocol_manager),
            client_tx,
            edge_tx,
            stop_tx: None,
        }
    }

    pub fn get_protocol_manager(&self) -> &ProtocolManager {
        &self.protocol_manager
    }

    pub fn init_protocols(
        &mut self,
        config: MessageServiceConfig,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let protocol_manager = Arc::get_mut(&mut self.protocol_manager)
            .ok_or_else(|| "Unable to get mutable reference to protocol manager".to_string())?;

        if config.enable_websocket {
            let ws_protocol = WebSocketProtocol::new(config.websocket_addr);
            protocol_manager.add_protocol(Box::new(ws_protocol));
            tracing::info!(
                "WebSocket protocol initialized on {}",
                config.websocket_addr
            );
        }

        if config.enable_tcp {
            let tcp_protocol = TcpProtocol::new(config.tcp_addr);
            protocol_manager.add_protocol(Box::new(tcp_protocol));
            tracing::info!("TCP protocol initialized on {}", config.tcp_addr);
        }

        Ok(())
    }

    /// Start the message service
    pub async fn start(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.protocol_manager.start_all().await?;

        let (stop_tx, mut stop_rx) = oneshot::channel();
        self.stop_tx = Some(stop_tx);

        let client_tx = self.client_tx.clone();
        let edge_tx = self.edge_tx.clone();
        let protocol_manager = self.protocol_manager.clone();
        let storage = self.storage.clone();

        tokio::spawn(async move {
            let mut app_rx = client_tx.subscribe();
            let mut device_rx = edge_tx.subscribe();

            loop {
                tokio::select! {
                    _ = &mut stop_rx => {
                        tracing::info!("Stopping message service");
                        break;
                    },
                    result = app_rx.recv() => {
                        if let Ok(message) = result {
                            if let Err(e) = protocol_manager.broadcast_app_message(message.clone()).await {
                                tracing::error!("Failed to broadcast app message: {}", e);
                            }

                            if let Err(e) = log_event(&storage, "client_message", &message).await {
                                tracing::error!("Failed to log client message: {}", e);
                            }
                        }
                    },
                    result = device_rx.recv() => {
                        if let Ok(message) = result {
                            if let Err(e) = protocol_manager.broadcast_device_message(message.clone()).await {
                                tracing::error!("Failed to broadcast device message: {}", e);
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the message service
    pub async fn stop(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }

        self.protocol_manager.stop_all().await?;

        Ok(())
    }

    /// Send application message to clients
    pub async fn send_app_message(
        &self,
        message: Message,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        log_event(&self.storage, "outgoing_app_message", &message).await?;

        self.client_tx
            .send(message)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        Ok(())
    }

    /// Send device message to edge devices
    pub async fn send_device_message(
        &self,
        message: Message,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        log_event(&self.storage, "outgoing_device_message", &message).await?;

        self.edge_tx
            .send(message)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        Ok(())
    }

    /// Process messages from Edge devices
    pub async fn process_edge_message(
        &self,
        message: Message,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        log_event(&self.storage, "edge_message", &message).await?;

        // Extract region ID from message source (Edge node ID)
        let region_id = match message.header.source {
            NodeId::Edge(edge_id) => edge_id as Id,
            _ => {
                tracing::warn!(
                    "Received edge message from invalid source: {:?}",
                    message.header.source
                );
                return Err("Invalid source for edge message".into());
            }
        };

        match &message.payload {
            MessagePayload::EdgeReport(report) => {
                self.handle_edge_report(region_id, report, &message).await?;
            }
            MessagePayload::Acknowledge(ack) => {
                tracing::info!(
                    "Received acknowledgment for message: {} - Status: {}",
                    ack.original_msg_id,
                    ack.status
                );
                if let Some(details) = &ack.details {
                    tracing::debug!("Acknowledgment details: {}", details);
                }
            }
            MessagePayload::Error(error) => {
                let original_msg = error
                    .original_msg_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "unknown".to_string());

                tracing::error!(
                    "Received error from edge device - Original Message: {}, Code: {:?}, Message: {}",
                    original_msg,
                    error.code,
                    error.message
                );

                // Handle specific error types
                match error.code {
                    ErrorCode::DeviceOffline => {
                        tracing::warn!("Device offline detected, may need to retry later");
                    }
                    ErrorCode::HardwareFailure => {
                        tracing::error!("Hardware failure detected, may need maintenance alert");
                    }
                    ErrorCode::BatteryLow => {
                        tracing::warn!("Low battery detected, monitoring device status");
                    }
                    _ => {
                        tracing::debug!("General error, no specific recovery action needed");
                    }
                }
            }
            _ => {
                tracing::warn!(
                    "Received unexpected message type from edge device: {:?}",
                    message.payload
                );
                return Err("Unexpected message type from edge device".into());
            }
        }

        Ok(())
    }

    /// Handle Edge device reports
    async fn handle_edge_report(
        &self,
        region_id: Id,
        report: &EdgeReport,
        original_message: &Message,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match report {
            EdgeReport::DeviceStatus { devices } => {
                if !self.region_exists(region_id).await? {
                    tracing::warn!(
                        "Received device status for non-existent region: {}",
                        region_id
                    );
                    return Ok(());
                }

                for (device_id, device_status) in devices {
                    if let Err(e) = self.update_device_status(*device_id, device_status).await {
                        tracing::error!("Failed to update device {} status: {}", device_id, e);
                    }
                }

                self.send_app_message(original_message.clone()).await?;

                tracing::debug!(
                    "Processed device status report for region {} with {} devices",
                    region_id,
                    devices.len()
                );
            }

            EdgeReport::HealthReport {
                cpu_usage,
                memory_usage,
            } => {
                tracing::info!(
                    "Edge device health report - CPU: {:.1}%, Memory: {:.1}%",
                    cpu_usage,
                    memory_usage
                );

                // Alert on high resource usage
                if *cpu_usage > 90.0 || *memory_usage > 90.0 {
                    tracing::warn!(
                        "Edge device resource usage is high - CPU: {:.1}%, Memory: {:.1}%",
                        cpu_usage,
                        memory_usage
                    );
                    // TODO: Could trigger alerts or scaling actions here
                }
            }

            EdgeReport::RequestTimeSync {
                local_time,
                current_offset_ms,
            } => {
                tracing::info!(
                    "Time sync request from edge device: local_time={}, offset={}ms",
                    local_time,
                    current_offset_ms
                );

                let time_sync_command = CloudCommand::TimeSync {
                    cloud_time: OffsetDateTime::now_utc(),
                };

                let message_id = self.send_control_message(time_sync_command).await?;
                tracing::info!(
                    "Time sync response sent to edge device (message_id: {})",
                    message_id
                );
            }
        }

        Ok(())
    }

    /// Send control message to Edge device
    pub async fn send_control_message(
        &self,
        command: CloudCommand,
    ) -> Result<Uuid, Box<dyn Error + Send + Sync>> {
        let message_id = Uuid::new_v4();

        let message = Message {
            header: MessageHeader {
                id: message_id,
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Cloud,
                target: NodeId::Edge(1), // TODO: Make target configurable
            },
            payload: MessagePayload::CloudCommand(command),
        };

        log_event(&self.storage, "cloud_command", &message).await?;

        self.send_app_message(message).await?;

        Ok(message_id)
    }

    /// Check if region exists in database
    async fn region_exists(&self, region_id: Id) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM regions WHERE id = $1")
            .bind(region_id as i32)
            .fetch_one(self.storage.get_pool())
            .await?;

        Ok(count > 0)
    }

    /// Update device status in database
    async fn update_device_status(
        &self,
        device_id: Id,
        device_status: &DeviceStatus,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let timestamp = device_status.updated_at;

        match &device_status.data {
            DeviceValue::Window { data } => {
                // Convert percentage (0-100) to state range (-1.0 to 1.0)
                let position_as_state = (data.target_position as f32 / 100.0) * 2.0 - 1.0;

                sqlx::query("UPDATE windows SET state = $1 WHERE id = $2")
                    .bind(position_as_state)
                    .bind(device_id as i32)
                    .execute(self.storage.get_pool())
                    .await?;

                tracing::debug!(
                    "Updated window {} position to {}% (state: {:.2})",
                    device_id,
                    data.target_position,
                    position_as_state
                );
            }

            DeviceValue::Sensor { data } => {
                let sensor_json = serde_json::json!({
                    "light": data.illuminance,
                    "temperature": data.temperature,
                    "humidity": data.humidity
                });

                sqlx::query(
                    "INSERT INTO device_records (device_id, data, time) VALUES ($1, $2, $3)",
                )
                .bind(device_id as i32)
                .bind(sensor_json)
                .bind(timestamp)
                .execute(self.storage.get_pool())
                .await?;

                tracing::debug!(
                    "Stored sensor data for device {}: temp={:.1}°C, humidity={:.1}%, light={}lux",
                    device_id,
                    data.temperature,
                    data.humidity,
                    data.illuminance
                );
            }
        }

        tracing::debug!(
            "Device {} status updated - Battery: {}%, RSSI: {}",
            device_id,
            device_status.battery,
            device_status.rssi
        );

        Ok(())
    }
}

/// Log message to database for audit trail
async fn log_event<T: serde::Serialize>(
    storage: &Storage,
    event_type: &str,
    message: &T,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let payload =
        serde_json::to_string(message).map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
    let timestamp = OffsetDateTime::now_utc();

    sqlx::query("INSERT INTO events (event_type, payload, time) VALUES ($1, $2, $3)")
        .bind(event_type)
        .bind(payload)
        .bind(timestamp)
        .execute(storage.get_pool())
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use lumisync_api::{
        Command, DeviceStatus, DeviceType, DeviceValue, Message, MessageHeader, MessagePayload,
        Priority, SensorData, UserRole, WindowData,
    };
    use serde_json::json;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::tests::*;

    use super::*;

    #[tokio::test]
    async fn test_message_service_initialization() {
        let storage = setup_test_db().await;
        let mut message_service = MessageService::new(storage);

        let config = MessageServiceConfig {
            // Use different ports to avoid conflicts between tests
            websocket_addr: "127.0.0.1:18080".parse().unwrap(),
            tcp_addr: "127.0.0.1:19000".parse().unwrap(),
            enable_websocket: true,
            enable_tcp: true,
        };

        let result = message_service.init_protocols(config);
        assert!(
            result.is_ok(),
            "Failed to initialize protocols: {:?}",
            result.err()
        );

        let start_result = message_service.start().await;
        assert!(
            start_result.is_ok(),
            "Failed to start service: {:?}",
            start_result.err()
        );

        let protocol_manager = message_service.get_protocol_manager();
        assert!(
            !protocol_manager.protocols.is_empty(),
            "Protocol manager should contain at least one protocol"
        );

        let stop_result = message_service.stop().await;
        assert!(
            stop_result.is_ok(),
            "Failed to stop service: {:?}",
            stop_result.err()
        );
    }

    #[tokio::test]
    async fn test_send_app_message() {
        let storage = setup_test_db().await;
        let mut message_service = MessageService::new(storage.clone());

        let config = MessageServiceConfig {
            // Use different ports to avoid conflicts between tests
            websocket_addr: "127.0.0.1:18081".parse().unwrap(),
            tcp_addr: "127.0.0.1:19001".parse().unwrap(),
            enable_websocket: true,
            enable_tcp: true,
        };

        message_service.init_protocols(config).unwrap();
        message_service.start().await.unwrap();

        let message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Cloud,
                target: NodeId::Edge(1),
            },
            payload: MessagePayload::CloudCommand(CloudCommand::ConfigureWindow {
                device_id: 1,
                plan: vec![],
            }),
        };

        let result = message_service.send_app_message(message).await;
        assert!(
            result.is_ok(),
            "Failed to send app message: {:?}",
            result.err()
        );

        message_service.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_send_device_message() {
        let storage = setup_test_db().await;
        let mut message_service = MessageService::new(storage.clone());

        let config = MessageServiceConfig {
            // Use different ports to avoid conflicts between tests
            websocket_addr: "127.0.0.1:18082".parse().unwrap(),
            tcp_addr: "127.0.0.1:19002".parse().unwrap(),
            enable_websocket: true,
            enable_tcp: true,
        };

        message_service.init_protocols(config).unwrap();
        message_service.start().await.unwrap();

        let device_message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Device([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]),
                target: NodeId::Cloud,
            },
            payload: MessagePayload::CloudCommand(CloudCommand::ControlDevices {
                commands: [(
                    1,
                    Command::SetWindow {
                        device_id: 1,
                        data: WindowData {
                            target_position: 75,
                        },
                    },
                )]
                .into_iter()
                .collect(),
            }),
        };

        let result = message_service.send_device_message(device_message).await;
        assert!(
            result.is_ok(),
            "Failed to send device message: {:?}",
            result.err()
        );

        message_service.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_process_edge_message() {
        let storage = setup_test_db().await;

        let _user = create_test_user(
            storage.clone(),
            "test@example.com",
            "password",
            &UserRole::User,
        )
        .await;
        let group = create_test_group(storage.clone(), "Test Group").await;
        let region = create_test_region(
            storage.clone(),
            group.id,
            "Test Region",
            500,
            25.0,
            50.0,
            true,
        )
        .await;

        let device = create_test_device(
            storage.clone(),
            region.id,
            "Test Device",
            &DeviceType::Sensor,
            json!({"state": "active"}),
        )
        .await;

        let mut message_service = MessageService::new(storage.clone());

        let config = MessageServiceConfig {
            websocket_addr: "127.0.0.1:18083".parse().unwrap(),
            tcp_addr: "127.0.0.1:19003".parse().unwrap(),
            enable_websocket: true,
            enable_tcp: true,
        };

        message_service.init_protocols(config).unwrap();
        message_service.start().await.unwrap();

        let sensor_data = SensorData {
            temperature: 24.5,
            humidity: 60.0,
            illuminance: 600,
        };

        let device_value = DeviceValue::Sensor { data: sensor_data };

        let device_status = DeviceStatus {
            data: device_value,
            battery: 100,
            rssi: 0,
            updated_at: OffsetDateTime::now_utc(),
        };

        let mut devices = BTreeMap::new();
        devices.insert(device.id, device_status);

        let edge_report = EdgeReport::DeviceStatus { devices };

        let message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(region.id as u8), // Use region.id as edge_id
                target: NodeId::Cloud,
            },
            payload: MessagePayload::EdgeReport(edge_report),
        };

        let result = message_service.process_edge_message(message).await;
        assert!(
            result.is_ok(),
            "Failed to process Edge message: {:?}",
            result.err()
        );

        message_service.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_send_control_message() {
        let storage = setup_test_db().await;
        let mut message_service = MessageService::new(storage.clone());

        let config = MessageServiceConfig {
            // Use different ports to avoid conflicts between tests
            websocket_addr: "127.0.0.1:18084".parse().unwrap(),
            tcp_addr: "127.0.0.1:19004".parse().unwrap(),
            enable_websocket: true,
            enable_tcp: true,
        };

        message_service.init_protocols(config).unwrap();
        message_service.start().await.unwrap();

        let command = CloudCommand::ConfigureWindow {
            device_id: 1,
            plan: vec![],
        };

        let result = message_service.send_control_message(command).await;
        assert!(
            result.is_ok(),
            "Failed to send control message: {:?}",
            result.err()
        );

        let message_id = result.unwrap();
        assert_ne!(message_id, Uuid::nil(), "Message ID should not be empty");

        message_service.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_time_sync_request_handling() {
        let storage = setup_test_db().await;
        let mut message_service = MessageService::new(storage.clone());

        let config = MessageServiceConfig {
            // Use different ports to avoid conflicts between tests
            websocket_addr: "127.0.0.1:18085".parse().unwrap(),
            tcp_addr: "127.0.0.1:19005".parse().unwrap(),
            enable_websocket: true,
            enable_tcp: true,
        };

        message_service.init_protocols(config).unwrap();
        message_service.start().await.unwrap();

        let edge_time = OffsetDateTime::from_unix_timestamp(1609459200).unwrap(); // 2021-01-01 00:00:00 UTC
        let time_sync_request = EdgeReport::RequestTimeSync {
            local_time: edge_time,
            current_offset_ms: -5000, // 5 seconds behind
        };

        let message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Cloud,
            },
            payload: MessagePayload::EdgeReport(time_sync_request),
        };

        let result = message_service.process_edge_message(message).await;
        assert!(
            result.is_ok(),
            "Failed to process time sync request: {:?}",
            result.err()
        );

        let event_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE event_type = 'edge_message'")
                .fetch_one(storage.get_pool())
                .await
                .unwrap();

        assert!(
            event_count > 0,
            "Time sync request should be logged in events"
        );

        message_service.stop().await.unwrap();
    }
}
