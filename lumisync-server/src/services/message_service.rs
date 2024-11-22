use std::net::SocketAddr;
use std::sync::Arc;

use lumisync_api::message::*;
use lumisync_api::{DeviceStatus, Id};
use time::OffsetDateTime;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::configs::Storage;
use crate::errors::{ApiError, MessageError};

use super::transport::MessageRouter;

type ServiceResult<T> = Result<T, ApiError>;

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
    message_router: Arc<MessageRouter>,
    incoming_rx: Option<mpsc::UnboundedReceiver<Message>>,
    stop_tx: Option<oneshot::Sender<()>>,
}

impl MessageService {
    pub fn new(
        storage: Arc<Storage>,
        message_router: Arc<MessageRouter>,
    ) -> (Self, mpsc::UnboundedSender<Message>) {
        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();

        let service = Self {
            storage,
            message_router,
            incoming_rx: Some(incoming_rx),
            stop_tx: None,
        };

        (service, incoming_tx)
    }

    pub async fn start(&mut self) -> ServiceResult<()> {
        let (stop_tx, mut stop_rx) = oneshot::channel();
        self.stop_tx = Some(stop_tx);

        let mut incoming_rx = self
            .incoming_rx
            .take()
            .ok_or(MessageError::AlreadyStarted)?;

        let storage = self.storage.clone();
        let message_router = self.message_router.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => {
                        tracing::info!("Message service stopped");
                        break;
                    },
                    message = incoming_rx.recv() => {
                        match message {
                            Some(msg) => {
                                if let Err(e) = Self::process_message(&storage, &message_router, msg).await {
                                    tracing::error!("Message processing failed: {}", e);
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        Ok(())
    }

    async fn process_message(
        storage: &Storage,
        message_router: &MessageRouter,
        message: Message,
    ) -> ServiceResult<()> {
        Self::audit_message(storage, &message).await?;

        match &message.payload {
            MessagePayload::EdgeReport(report) => {
                Self::process_edge_report(storage, message_router, &message, report).await?;
            }
            MessagePayload::CloudCommand(_) => {
                message_router.publish_device_message(message).await;
            }
            MessagePayload::EdgeCommand(_) => {
                message_router.publish_device_message(message).await;
            }
            MessagePayload::DeviceReport(_) => {
                message_router.publish_app_message(message).await;
            }
            MessagePayload::TimeSync(sync_payload) => {
                Self::handle_time_sync(storage, message_router, &message, sync_payload).await?;
            }
            MessagePayload::Acknowledge(_) | MessagePayload::Error(_) => {
                message_router.publish_app_message(message).await;
            }
        }

        Ok(())
    }

    async fn process_edge_report(
        storage: &Storage,
        message_router: &MessageRouter,
        original_message: &Message,
        report: &EdgeReport,
    ) -> ServiceResult<()> {
        match report {
            EdgeReport::DeviceStatus { devices } => {
                Self::handle_device_status_update(
                    storage,
                    message_router,
                    original_message,
                    devices,
                )
                .await?;
            }
            EdgeReport::HealthReport {
                cpu_usage,
                memory_usage,
            } => {
                Self::handle_health_report(*cpu_usage, *memory_usage).await;
            }
        }

        Ok(())
    }

    async fn handle_device_status_update(
        storage: &Storage,
        message_router: &MessageRouter,
        original_message: &Message,
        devices: &std::collections::BTreeMap<Id, DeviceStatus>,
    ) -> ServiceResult<()> {
        let mut tx = storage.get_pool().begin().await?;

        for (device_id, status) in devices {
            Self::persist_device_record(&mut tx, *device_id, status).await?;
            Self::update_device_status(&mut tx, *device_id, status).await?;
        }

        tx.commit().await?;
        message_router
            .publish_app_message(original_message.clone())
            .await;

        Ok(())
    }

    async fn persist_device_record(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        device_id: Id,
        status: &DeviceStatus,
    ) -> ServiceResult<()> {
        let data = serde_json::to_value(&status).map_err(MessageError::Serialization)?;

        sqlx::query("INSERT INTO device_records (device_id, data, time) VALUES (?, ?, ?)")
            .bind(device_id)
            .bind(data)
            .bind(status.updated_at)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    async fn update_device_status(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        device_id: Id,
        status: &DeviceStatus,
    ) -> ServiceResult<()> {
        let status_json = serde_json::to_value(status).map_err(MessageError::Serialization)?;

        sqlx::query("UPDATE devices SET status = ? WHERE id = ?")
            .bind(&status_json)
            .bind(device_id)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    async fn handle_health_report(cpu_usage: f32, memory_usage: f32) {
        tracing::info!(
            target: "edge_health",
            cpu_percent = cpu_usage,
            memory_percent = memory_usage,
            "Edge health report received"
        );
    }

    async fn handle_time_sync(
        storage: &Storage,
        message_router: &MessageRouter,
        message: &Message,
        sync_payload: &TimeSyncPayload,
    ) -> ServiceResult<()> {
        match sync_payload {
            TimeSyncPayload::Request { sequence, .. } => {
                if matches!(message.header.source, NodeId::Edge(_)) {
                    Self::respond_to_edge_time_request(message_router, message, *sequence).await;
                } else {
                    tracing::warn!(
                        "Rejected time sync request from {:?} - only Edge nodes allowed",
                        message.header.source
                    );
                }
            }
            TimeSyncPayload::StatusQuery => {
                Self::respond_to_status_query(message_router, message).await;
            }
            _ => {
                tracing::debug!("Cloud ignoring time sync message type: {:?}", sync_payload);
            }
        }

        Self::audit_time_sync(storage, message).await?;
        Ok(())
    }

    async fn respond_to_edge_time_request(
        message_router: &MessageRouter,
        request: &Message,
        sequence: u32,
    ) {
        let now = OffsetDateTime::now_utc();
        
        let response = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: now,
                priority: Priority::Emergency,
                source: NodeId::Cloud,
                target: request.header.source,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Response {
                request_sequence: sequence,
                request_receive_time: request.header.timestamp,
                response_send_time: now,
                estimated_delay_ms: 25, // Typical delay from Cloud to Edge
                accuracy_ms: 1,         // Cloud provides 1ms accuracy
            }),
        };

        message_router.publish_device_message(response).await;
        
        tracing::debug!(
            target: "time_sync",
            edge = ?request.header.source,
            sequence = sequence,
            "Provided authoritative time to Edge"
        );
    }

    async fn respond_to_status_query(message_router: &MessageRouter, query: &Message) {
        let now = OffsetDateTime::now_utc();
        
        let response = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: now,
                priority: Priority::Regular,
                source: NodeId::Cloud,
                target: query.header.source,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::StatusResponse {
                is_synced: true,        // Cloud is the authoritative source, always synced
                current_offset_ms: 0,   // Cloud offset is 0
                last_sync_time: now,
                accuracy_ms: 1,
            }),
        };

        message_router.publish_device_message(response).await;
    }

    async fn audit_message(storage: &Storage, message: &Message) -> ServiceResult<()> {
        let event_data = serde_json::json!({
            "message_id": message.header.id,
            "source": message.header.source,
            "target": message.header.target,
            "priority": message.header.priority,
            "payload_type": Self::get_payload_type(&message.payload)
        });

        sqlx::query("INSERT INTO events (event_type, payload, time) VALUES (?, ?, ?)")
            .bind("message_received")
            .bind(event_data)
            .bind(message.header.timestamp)
            .execute(storage.get_pool())
            .await?;

        Ok(())
    }

    async fn audit_time_sync(storage: &Storage, message: &Message) -> ServiceResult<()> {
        let event_data = serde_json::json!({
            "message_id": message.header.id,
            "source": message.header.source,
            "payload_type": "time_sync"
        });

        sqlx::query("INSERT INTO events (event_type, payload, time) VALUES (?, ?, ?)")
            .bind("time_sync_handled")
            .bind(event_data)
            .bind(message.header.timestamp)
            .execute(storage.get_pool())
            .await?;

        Ok(())
    }

    fn get_payload_type(payload: &MessagePayload) -> &'static str {
        match payload {
            MessagePayload::EdgeReport(_) => "edge_report",
            MessagePayload::CloudCommand(_) => "cloud_command",
            MessagePayload::EdgeCommand(_) => "edge_command",
            MessagePayload::DeviceReport(_) => "device_report",
            MessagePayload::TimeSync(_) => "time_sync",
            MessagePayload::Acknowledge(_) => "acknowledge",
            MessagePayload::Error(_) => "error",
        }
    }

    pub async fn stop(&mut self) -> ServiceResult<()> {
        if let Some(tx) = self.stop_tx.take() {
            tx.send(()).map_err(|_| MessageError::ChannelClosed)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use lumisync_api::message::*;
    use lumisync_api::{DeviceStatus, DeviceType, DeviceValue, SensorData, UserRole};
    use serde_json::json;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::tests::*;

    use super::*;

    async fn setup_test_service() -> (Arc<Storage>, MessageService, mpsc::UnboundedSender<Message>)
    {
        let storage = setup_test_db().await;
        let (message_tx, _) = mpsc::unbounded_channel();
        let message_router = Arc::new(MessageRouter::new(message_tx));
        let (service, incoming_tx) = MessageService::new(storage.clone(), message_router);

        (storage, service, incoming_tx)
    }

    #[tokio::test]
    async fn test_service_lifecycle() {
        let (_, mut service, _) = setup_test_service().await;

        assert!(service.start().await.is_ok());
        assert!(service.stop().await.is_ok());
    }

    #[tokio::test]
    async fn test_edge_time_sync_request() {
        let (_, mut service, incoming_tx) = setup_test_service().await;
        service.start().await.unwrap();

        let request = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Emergency,
                source: NodeId::Edge(1),
                target: NodeId::Cloud,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Request {
                sequence: 1,
                send_time: OffsetDateTime::now_utc(),
                precision_ms: 10,
            }),
        };

        assert!(incoming_tx.send(request).is_ok());
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        service.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_device_time_sync_rejected() {
        let (_, mut service, incoming_tx) = setup_test_service().await;
        service.start().await.unwrap();

        let request = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Emergency,
                source: NodeId::Device([1, 2, 3, 4, 5, 6]),
                target: NodeId::Cloud,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::Request {
                sequence: 1,
                send_time: OffsetDateTime::now_utc(),
                precision_ms: 50,
            }),
        };

        assert!(incoming_tx.send(request).is_ok());
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        service.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_time_status_query() {
        let (_, mut service, incoming_tx) = setup_test_service().await;
        service.start().await.unwrap();

        let query = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Cloud,
            },
            payload: MessagePayload::TimeSync(TimeSyncPayload::StatusQuery),
        };

        assert!(incoming_tx.send(query).is_ok());
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        service.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_device_status_processing() {
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

        let (_, mut service, incoming_tx) = setup_test_service().await;
        service.start().await.unwrap();

        let device_status = DeviceStatus {
            data: DeviceValue::Sensor {
                data: SensorData {
                    temperature: 24.5,
                    humidity: 60.0,
                    illuminance: 600,
                },
            },
            battery: 100,
            rssi: -45,
            updated_at: OffsetDateTime::now_utc(),
        };

        let mut devices = BTreeMap::new();
        devices.insert(device.id, device_status);

        let message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(region.id as u8),
                target: NodeId::Cloud,
            },
            payload: MessagePayload::EdgeReport(EdgeReport::DeviceStatus { devices }),
        };

        assert!(incoming_tx.send(message).is_ok());
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        service.stop().await.unwrap();
    }
}
