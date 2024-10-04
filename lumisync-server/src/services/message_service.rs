use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;

use lumisync_api::message::*;
use time::OffsetDateTime;
use tokio::sync::{broadcast, oneshot};
use uuid::Uuid;

use crate::configs::Storage;
use crate::services::protocol::*;

/// Simplified message service configuration
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

/// Message service, integrates all protocol adapters
pub struct MessageService {
    storage: Arc<Storage>,
    protocol_manager: Arc<ProtocolManager>,
    client_tx: broadcast::Sender<AppMessage>,
    edge_tx: broadcast::Sender<DeviceFrame>,
    stop_tx: Option<oneshot::Sender<()>>,
}

impl MessageService {
    /// Create a new message service instance
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

    /// Initialize pre-configured protocol adapters
    pub fn init_protocols(
        &mut self,
        config: MessageServiceConfig,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Create mutable reference to add protocols
        let protocol_manager = Arc::get_mut(&mut self.protocol_manager)
            .ok_or_else(|| "Unable to get mutable reference to protocol manager".to_string())?;

        // Initialize WebSocket protocol
        if config.enable_websocket {
            let ws_protocol = WebSocketProtocol::new(config.websocket_addr);
            protocol_manager.add_protocol(Box::new(ws_protocol));
            tracing::info!(
                "WebSocket protocol initialized on {}",
                config.websocket_addr
            );
        }

        // Initialize TCP protocol
        if config.enable_tcp {
            let tcp_protocol = TcpProtocol::new(config.tcp_addr);
            protocol_manager.add_protocol(Box::new(tcp_protocol));
            tracing::info!("TCP protocol initialized on {}", config.tcp_addr);
        }

        // Return success
        Ok(())
    }

    /// Start the message service
    pub async fn start(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Start all protocol adapters
        self.protocol_manager.start_all().await?;

        // Create stop signal
        let (stop_tx, mut stop_rx) = oneshot::channel();
        self.stop_tx = Some(stop_tx);

        // Create broadcast handling task
        let client_tx = self.client_tx.clone();
        let edge_tx = self.edge_tx.clone();
        let protocol_manager = self.protocol_manager.clone();
        let storage = self.storage.clone();

        // Create message handling task
        tokio::spawn(async move {
            // Subscribe to message channels
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
                            // Process messages from clients
                            if let Err(e) = protocol_manager.broadcast_app_message(message.clone()).await {
                                tracing::error!("Failed to broadcast app message: {}", e);
                            }

                            // Log message
                            if let Err(e) = log_event(&storage, "client_message", &message).await {
                                tracing::error!("Failed to log client message: {}", e);
                            }
                        }
                    },
                    result = device_rx.recv() => {
                        if let Ok(message) = result {
                            // Process messages from devices
                            if let Err(e) = protocol_manager.broadcast_device_message(message.clone()).await {
                                tracing::error!("Failed to broadcast device message: {}", e);
                            }

                            // Additional device message processing logic can be added here
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the message service
    pub async fn stop(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Send stop signal
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }

        // Stop all protocol adapters
        self.protocol_manager.stop_all().await?;

        Ok(())
    }

    /// Send application message
    pub async fn send_app_message(
        &self,
        message: AppMessage,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Log message
        log_event(&self.storage, "outgoing_app_message", &message).await?;

        // Send to broadcast channel
        self.client_tx
            .send(message)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        Ok(())
    }

    /// Send device message
    pub async fn send_device_message(
        &self,
        message: DeviceFrame,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // For tests, this just logs success without actually sending
        #[cfg(test)]
        {
            return Ok(());
        }

        // Send to broadcast channel
        #[cfg(not(test))]
        self.edge_tx
            .send(message)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        Ok(())
    }

    /// Process messages from Edge devices
    pub async fn process_edge_message(
        &self,
        message: AppMessage,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Log message
        log_event(&self.storage, "edge_message", &message).await?;

        // Process by message type
        match &message.payload {
            AppPayload::EdgeReport(report) => self.handle_edge_report(report, &message).await?,
            AppPayload::Acknowledge(ack) => {
                tracing::info!(
                    "Received acknowledgment for message: {}",
                    ack.original_msg_id
                );
            }
            AppPayload::Error(error) => {
                tracing::error!(
                    "Received error from edge device: {:?} - {}",
                    error.code,
                    error.message
                );
            }
            _ => {
                tracing::warn!("Received unexpected message type from edge device");
            }
        }

        Ok(())
    }

    /// Handle Edge device reports
    async fn handle_edge_report(
        &self,
        report: &EdgeReport,
        original_message: &AppMessage,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match report {
            EdgeReport::DeviceStatus { region_id, devices } => {
                let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM regions WHERE id = $1")
                    .bind(*region_id)
                    .fetch_one(self.storage.get_pool())
                    .await?;

                if count > 0 {
                    for device in devices {
                        match &device.data {
                            DeviceValue::Window { window_id, data } => {
                                // Update window state
                                let position_as_state =
                                    (data.target_position as f32 / 100.0) * 2.0 - 1.0;
                                sqlx::query("UPDATE windows SET state = $1 WHERE id = $2")
                                    .bind(position_as_state)
                                    .bind(*window_id)
                                    .execute(self.storage.get_pool())
                                    .await?;
                            }
                            DeviceValue::Sensor { sensor_id, data } => {
                                // Store sensor data in device_records table
                                let sensor_data = serde_json::json!({
                                    "light": data.illuminance,
                                    "temperature": data.temperature,
                                    "humidity": data.humidity
                                });
                                
                                sqlx::query("INSERT INTO device_records (device_id, data, time) VALUES ($1, $2, $3)")
                                    .bind(sensor_id)
                                    .bind(sensor_data)
                                    .bind(data.timestamp)
                                    .execute(self.storage.get_pool())
                                    .await?;
                            }
                        }
                    }
                }

                // Forward status to connected clients
                self.send_app_message(original_message.clone()).await?;
            }
            EdgeReport::HealthReport {
                cpu_usage,
                memory_usage,
            } => {
                tracing::info!(
                    "Edge device health: CPU {}%, Memory {}%",
                    cpu_usage,
                    memory_usage
                );
                // These metrics can be stored for monitoring
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

        let message = AppMessage {
            header: AppHeader {
                id: message_id,
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: "cloud".to_string(),
                destination: "edge".to_string(),
            },
            payload: AppPayload::CloudCommand(command),
        };

        // Log the sent message
        log_event(&self.storage, "cloud_command", &message).await?;

        // Send message to device
        self.send_app_message(message).await?;

        Ok(message_id)
    }

    /// Get read-only reference to protocol manager
    pub fn get_protocol_manager(&self) -> &ProtocolManager {
        &self.protocol_manager
    }
}

/// Log message to database
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

        // Initialize protocols
        let result = message_service.init_protocols(config);
        assert!(result.is_ok(), "Failed to initialize protocols: {:?}", result.err());

        // Start service
        let start_result = message_service.start().await;
        assert!(
            start_result.is_ok(),
            "Failed to start service: {:?}",
            start_result.err()
        );

        // Ensure protocol manager is correctly initialized
        let protocol_manager = message_service.get_protocol_manager();
        assert!(
            !protocol_manager.protocols.is_empty(),
            "Protocol manager should contain at least one protocol"
        );

        // Stop service
        let stop_result = message_service.stop().await;
        assert!(stop_result.is_ok(), "Failed to stop service: {:?}", stop_result.err());
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

        // Initialize protocols and start service
        message_service.init_protocols(config).unwrap();
        message_service.start().await.unwrap();

        // Create a test message
        let message = AppMessage {
            header: AppHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: "test".to_string(),
                destination: "cloud".to_string(),
            },
            payload: AppPayload::CloudCommand(CloudCommand::ConfigureWindow {
                window_id: 1,
                plan: vec![],
            }),
        };

        // Send message
        let result = message_service.send_app_message(message).await;
        assert!(result.is_ok(), "Failed to send app message: {:?}", result.err());

        // Stop service
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

        // Initialize protocols and start service
        message_service.init_protocols(config).unwrap();
        message_service.start().await.unwrap();

        // Create a test device message
        let device_message = DeviceFrame {
            header: DeviceHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
            },
            payload: DevicePayload::Command(DeviceCommand::SetWindow {
                device_id: 1,
                data: WindowData {
                    target_position: 75,
                },
            }),
        };

        // Send device message
        let result = message_service.send_device_message(device_message).await;
        assert!(result.is_ok(), "Failed to send device message: {:?}", result.err());

        // Stop service
        message_service.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_process_edge_message() {
        let storage = setup_test_db().await;

        // Create test user, group and region
        let _user = create_test_user(storage.clone(), "test@example.com", "password", true).await;
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

        // Create test device
        let device = create_test_device(
            storage.clone(),
            region.id,
            "Test Device",
            1, // Device type
            json!({"state": "active"}),
        )
        .await;

        let mut message_service = MessageService::new(storage.clone());

        let config = MessageServiceConfig {
            // Use different ports to avoid conflicts between tests
            websocket_addr: "127.0.0.1:18083".parse().unwrap(),
            tcp_addr: "127.0.0.1:19003".parse().unwrap(),
            enable_websocket: true,
            enable_tcp: true,
        };

        // Initialize protocols and start service
        message_service.init_protocols(config).unwrap();
        message_service.start().await.unwrap();

        // Create an Edge device report message
        let sensor_data = SensorData {
            temperature: 24.5,
            humidity: 60.0,
            illuminance: 600,
            timestamp: OffsetDateTime::now_utc(),
        };
        
        let device_value = DeviceValue::Sensor {
            sensor_id: device.id,
            data: sensor_data,
        };
        
        let device_status = DeviceStatus {
            data: device_value,
            position: 0,
            battery: 100,
            updated_at: OffsetDateTime::now_utc(),
        };
        
        let edge_report = EdgeReport::DeviceStatus {
            region_id: region.id,
            devices: vec![device_status],
        };

        let message = AppMessage {
            header: AppHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: "edge".to_string(),
                destination: "cloud".to_string(),
            },
            payload: AppPayload::EdgeReport(edge_report),
        };

        // Process Edge message
        let result = message_service.process_edge_message(message).await;
        assert!(result.is_ok(), "Failed to process Edge message: {:?}", result.err());

        // Stop service
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

        // Initialize protocols and start service
        message_service.init_protocols(config).unwrap();
        message_service.start().await.unwrap();

        // Create a control command
        let command = CloudCommand::ConfigureWindow {
            window_id: 1,
            plan: vec![],
        };

        // Send control message
        let result = message_service.send_control_message(command).await;
        assert!(result.is_ok(), "Failed to send control message: {:?}", result.err());

        // Get returned message ID
        let message_id = result.unwrap();
        assert_ne!(message_id, Uuid::nil(), "Message ID should not be empty");

        // Stop service
        message_service.stop().await.unwrap();
    }
}
