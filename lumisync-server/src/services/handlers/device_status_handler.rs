use std::collections::BTreeMap;
use std::sync::Arc;

use lumisync_api::handler::{MessageError, MessageHandler, PayloadType};
use lumisync_api::message::*;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::configs::Storage;

pub struct DeviceStatusHandler {
    /// Database storage
    storage: Arc<Storage>,
    /// Device status cache
    device_cache: Arc<Mutex<BTreeMap<Id, DeviceStatus>>>,
}

impl DeviceStatusHandler {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            storage,
            device_cache: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    async fn handle_device_status_update(
        &mut self,
        _message: &Message,
        devices: &BTreeMap<Id, DeviceStatus>,
    ) -> Result<Option<Message>, MessageError> {
        {
            let mut cache = self.device_cache.lock().await;
            for (device_id, status) in devices {
                cache.insert(*device_id, status.clone());
                info!(
                    "Device {} status updated: battery {}%, signal {}dBm",
                    device_id, status.battery, status.rssi
                );
            }
        }

        for (device_id, status) in devices {
            if let Err(e) = self.save_device_status(*device_id, status).await {
                error!("Failed to save device {} status: {}", device_id, e);
            }
        }

        debug!("Processed status updates for {} devices", devices.len());
        Ok(None)
    }

    async fn handle_health_report(
        &self,
        message: &Message,
        cpu_usage: f32,
        memory_usage: f32,
    ) -> Result<Option<Message>, MessageError> {
        if let NodeId::Edge(edge_id) = message.header.source {
            info!(
                "Edge node {} health status: CPU {}%, memory {}%",
                edge_id, cpu_usage, memory_usage
            );

            if cpu_usage > 80.0 {
                warn!(
                    "Edge node {} CPU usage too high: {:.1}%",
                    edge_id, cpu_usage
                );
            }
            if memory_usage > 90.0 {
                warn!(
                    "Edge node {} memory usage too high: {:.1}%",
                    edge_id, memory_usage
                );
            }

            self.save_edge_health(edge_id, cpu_usage, memory_usage)
                .await;
        }

        Ok(None)
    }

    async fn save_device_status(&self, device_id: Id, status: &DeviceStatus) -> Result<(), String> {
        let status_json = serde_json::to_value(status)
            .map_err(|e| format!("Status serialization failed: {}", e))?;

        if let Err(e) =
            sqlx::query("INSERT INTO device_records (device_id, data, time) VALUES (?, ?, ?)")
                .bind(device_id)
                .bind(&status_json)
                .bind(status.updated_at)
                .execute(self.storage.get_pool())
                .await
        {
            return Err(format!("Device record insertion failed: {}", e));
        }

        if let Err(e) = sqlx::query("UPDATE devices SET status = ?, last_seen = ? WHERE id = ?")
            .bind(&status_json)
            .bind(status.updated_at)
            .bind(device_id)
            .execute(self.storage.get_pool())
            .await
        {
            return Err(format!("Device status update failed: {}", e));
        }

        Ok(())
    }

    async fn save_edge_health(&self, edge_id: u8, cpu_usage: f32, memory_usage: f32) {
        let health_data = serde_json::json!({
            "edge_id": edge_id,
            "cpu_usage": cpu_usage,
            "memory_usage": memory_usage
        });

        if let Err(e) =
            sqlx::query("INSERT INTO edge_health (edge_id, health_data, time) VALUES (?, ?, ?)")
                .bind(edge_id)
                .bind(health_data)
                .bind(time::OffsetDateTime::now_utc())
                .execute(self.storage.get_pool())
                .await
        {
            error!("Failed to record edge node health status: {}", e);
        }
    }

    pub async fn get_device_stats(&self) -> DeviceStatusStats {
        let cache = self.device_cache.lock().await;

        let total_devices = cache.len();
        let low_battery_devices = cache.values().filter(|status| status.battery < 20).count();
        let weak_signal_devices = cache.values().filter(|status| status.rssi < -80).count();

        let avg_battery = if total_devices > 0 {
            cache
                .values()
                .map(|status| status.battery as f64)
                .sum::<f64>()
                / total_devices as f64
        } else {
            0.0
        };

        DeviceStatusStats {
            total_devices,
            low_battery_devices,
            weak_signal_devices,
            average_battery_level: avg_battery,
        }
    }
}

impl MessageHandler for DeviceStatusHandler {
    fn handle_message(&mut self, message: Message) -> Result<Option<Message>, MessageError> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| MessageError::InternalError(format!("Failed to create runtime: {}", e)))?;

        rt.block_on(async {
            match &message.payload {
                MessagePayload::EdgeReport(report) => match report {
                    EdgeReport::DeviceStatus { devices } => {
                        self.handle_device_status_update(&message, devices).await
                    }
                    EdgeReport::HealthReport {
                        cpu_usage,
                        memory_usage,
                    } => {
                        self.handle_health_report(&message, *cpu_usage, *memory_usage)
                            .await
                    }
                },
                _ => Ok(None),
            }
        })
    }

    fn supported_payloads(&self) -> Vec<PayloadType> {
        vec![PayloadType::EdgeReport]
    }

    fn node_id(&self) -> NodeId {
        NodeId::Cloud
    }

    fn name(&self) -> &'static str {
        "DeviceStatusHandler"
    }
}

#[derive(Debug, Clone)]
pub struct DeviceStatusStats {
    pub total_devices: usize,
    pub low_battery_devices: usize,
    pub weak_signal_devices: usize,
    pub average_battery_level: f64,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use lumisync_api::message::{EdgeReport, MessageHeader, Priority};
    use lumisync_api::models::{DeviceStatus, DeviceValue, SensorData};
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::tests::setup_test_db;

    use super::*;

    #[tokio::test]
    async fn test_device_status_processing() {
        let storage = setup_test_db().await;
        let mut handler = DeviceStatusHandler::new(storage.clone());

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
        devices.insert(1, device_status);

        let message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Cloud,
            },
            payload: MessagePayload::EdgeReport(EdgeReport::DeviceStatus {
                devices: devices.clone(),
            }),
        };

        let result = handler
            .handle_device_status_update(&message, &devices)
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_health_report_processing() {
        let storage = setup_test_db().await;
        let handler = DeviceStatusHandler::new(storage.clone());

        let message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Edge(1),
                target: NodeId::Cloud,
            },
            payload: MessagePayload::EdgeReport(EdgeReport::HealthReport {
                cpu_usage: 45.0,
                memory_usage: 60.0,
            }),
        };

        let result = handler.handle_health_report(&message, 45.0, 60.0).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_device_stats_calculation() {
        let storage = setup_test_db().await;
        let handler = DeviceStatusHandler::new(storage.clone());

        // Add some test devices to cache
        {
            let mut cache = handler.device_cache.lock().await;

            // Device with low battery
            cache.insert(
                1,
                DeviceStatus {
                    data: DeviceValue::Sensor {
                        data: SensorData {
                            temperature: 24.0,
                            humidity: 60.0,
                            illuminance: 500,
                        },
                    },
                    battery: 15, // Low battery
                    rssi: -50,
                    updated_at: OffsetDateTime::now_utc(),
                },
            );

            // Device with weak signal
            cache.insert(
                2,
                DeviceStatus {
                    data: DeviceValue::Sensor {
                        data: SensorData {
                            temperature: 25.0,
                            humidity: 65.0,
                            illuminance: 600,
                        },
                    },
                    battery: 80,
                    rssi: -85, // Weak signal
                    updated_at: OffsetDateTime::now_utc(),
                },
            );

            // Normal device
            cache.insert(
                3,
                DeviceStatus {
                    data: DeviceValue::Sensor {
                        data: SensorData {
                            temperature: 23.0,
                            humidity: 55.0,
                            illuminance: 700,
                        },
                    },
                    battery: 90,
                    rssi: -60,
                    updated_at: OffsetDateTime::now_utc(),
                },
            );
        }

        let stats = handler.get_device_stats().await;

        assert_eq!(stats.total_devices, 3);
        assert_eq!(stats.low_battery_devices, 1);
        assert_eq!(stats.weak_signal_devices, 1);
        assert!((stats.average_battery_level - 61.67).abs() < 0.1); // (15 + 80 + 90) / 3 ≈ 61.67
    }
}
