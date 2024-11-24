use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use lumisync_api::message::*;
use lumisync_api::time::{SyncConfig, TimeProvider, TimeSyncService};
use lumisync_api::uuid::RandomUuidGenerator;
use lumisync_api::{DeviceStatus, Id};
use time::OffsetDateTime;
use tokio::sync::{mpsc, oneshot};

use crate::configs::Storage;
use crate::errors::{ApiError, MessageError};

use super::transport::MessageRouter;

type RandomTimeSyncService = TimeSyncService<SystemTimeProvider, RandomUuidGenerator>;

#[derive(Debug, Default, Clone)]
pub struct SystemTimeProvider;

impl TimeProvider for SystemTimeProvider {
    fn monotonic_time_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn wall_clock_time(&self) -> Option<OffsetDateTime> {
        Some(OffsetDateTime::now_utc())
    }

    fn has_authoritative_time(&self) -> bool {
        true
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

    fn create_time_sync_service() -> RandomTimeSyncService {
        let config = SyncConfig {
            sync_interval_ms: 60000, // 1 minute - server doesn't need frequent sync
            max_drift_ms: 100,       // 100ms tolerance
            offset_history_size: 10,
            delay_threshold_ms: 1000, // 1 second tolerance for network delay
            max_retry_count: 3,
            failure_cooldown_ms: 30000, // 30 seconds cooldown
        };

        let time_provider = SystemTimeProvider::default();

        TimeSyncService::new(time_provider, NodeId::Cloud, config, RandomUuidGenerator)
    }

    pub async fn start(&mut self) -> Result<(), ApiError> {
        let (stop_tx, mut stop_rx) = oneshot::channel();
        self.stop_tx = Some(stop_tx);

        let mut incoming_rx = self
            .incoming_rx
            .take()
            .ok_or(MessageError::AlreadyStarted)?;

        let storage = self.storage.clone();
        let message_router = self.message_router.clone();
        let mut time_service = Self::create_time_sync_service();

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
                                if let Err(e) = Self::process_message(&storage, &message_router, &mut time_service, msg).await {
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
        time_service: &mut RandomTimeSyncService,
        message: Message,
    ) -> Result<(), ApiError> {
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
                Self::handle_time_sync(
                    storage,
                    message_router,
                    time_service,
                    &message,
                    sync_payload,
                )
                .await?;
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
    ) -> Result<(), ApiError> {
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
        devices: &BTreeMap<Id, DeviceStatus>,
    ) -> Result<(), ApiError> {
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
    ) -> Result<(), ApiError> {
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
    ) -> Result<(), ApiError> {
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
        time_service: &mut RandomTimeSyncService,
        message: &Message,
        sync_payload: &TimeSyncPayload,
    ) -> Result<(), ApiError> {
        match sync_payload {
            TimeSyncPayload::Request { .. } => {
                if matches!(message.header.source, NodeId::Edge(_)) {
                    match time_service.handle_sync_request(message) {
                        Ok(response) => {
                            message_router.publish_device_message(response).await;
                            tracing::debug!(
                                target: "time_sync",
                                edge = ?message.header.source,
                                "Provided authoritative time to Edge"
                            );
                        }
                        Err(e) => tracing::error!("Failed to handle time sync request: {}", e),
                    }
                } else {
                    tracing::warn!(
                        "Rejected time sync request from {:?} - only Edge nodes allowed",
                        message.header.source
                    );
                }
            }
            TimeSyncPayload::StatusQuery => match time_service.handle_status_query(message) {
                Ok(response) => {
                    message_router.publish_device_message(response).await;
                }
                Err(e) => {
                    tracing::error!("Failed to handle time status query: {}", e);
                }
            },
            _ => tracing::debug!("Cloud ignoring time sync message type: {:?}", sync_payload),
        }

        Self::audit_time_sync(storage, message).await?;
        Ok(())
    }

    async fn audit_message(storage: &Storage, message: &Message) -> Result<(), ApiError> {
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

    async fn audit_time_sync(storage: &Storage, message: &Message) -> Result<(), ApiError> {
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

    pub async fn stop(&mut self) -> Result<(), ApiError> {
        if let Some(tx) = self.stop_tx.take() {
            tx.send(()).map_err(|_| MessageError::ChannelClosed)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
                send_time: Some(OffsetDateTime::now_utc()),
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
                send_time: Some(OffsetDateTime::now_utc()),
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
