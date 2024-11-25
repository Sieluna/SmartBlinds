use lumisync_api::handler::{MessageError, MessageHandler, PayloadType};
use lumisync_api::message::*;
use lumisync_api::models::{Id, SensorData, WindowData};
use std::sync::Arc;
use tracing::info;

pub struct AnalyticsHandler {
    _storage: Arc<crate::configs::Storage>,
}

impl AnalyticsHandler {
    pub fn new(storage: Arc<crate::configs::Storage>) -> Self {
        Self { _storage: storage }
    }

    async fn handle_device_report(
        &mut self,
        _message: &Message,
        device_report: &DeviceReport,
    ) -> Result<Option<Message>, MessageError> {
        match device_report {
            DeviceReport::SensorData { sensor_data, .. } => {
                if let NodeId::Device(mac_addr) = _message.header.source {
                    let device_id = self.get_device_id_from_mac(mac_addr).await?;
                    let recommendation = self.analyze_and_recommend(device_id, sensor_data).await?;

                    if let Some(rec) = recommendation {
                        info!(
                            "Device {} analysis recommendation: position {}%, reason: {}",
                            device_id, rec.position, rec.reason
                        );
                        return Ok(Some(self.create_recommendation_message(rec)));
                    }
                }
            }
            _ => {}
        }
        Ok(None)
    }

    async fn analyze_and_recommend(
        &self,
        device_id: Id,
        sensor_data: &SensorData,
    ) -> Result<Option<Recommendation>, MessageError> {
        let mut recommended_position = 50u8;
        let mut reasons = Vec::new();

        if sensor_data.temperature > 28.0 {
            recommended_position = 30;
            reasons.push("High temperature, recommend shading");
        } else if sensor_data.temperature < 18.0 {
            recommended_position = 80;
            reasons.push("Low temperature, recommend more sunlight");
        }

        if sensor_data.illuminance > 2000 {
            recommended_position = recommended_position.min(20);
            reasons.push("Strong light, recommend anti-glare");
        } else if sensor_data.illuminance < 300 {
            recommended_position = recommended_position.max(70);
            reasons.push("Insufficient light, recommend more natural light");
        }

        if sensor_data.humidity > 70.0 {
            recommended_position = recommended_position.max(60);
            reasons.push("High humidity, recommend ventilation");
        }

        if !reasons.is_empty() {
            Ok(Some(Recommendation {
                device_id,
                position: recommended_position,
                reason: reasons.join("; "),
            }))
        } else {
            Ok(None)
        }
    }

    fn create_recommendation_message(&self, rec: Recommendation) -> Message {
        let mut windows = std::collections::BTreeMap::new();
        windows.insert(
            rec.device_id,
            WindowData {
                target_position: rec.position,
            },
        );

        Message {
            header: MessageHeader {
                id: uuid::Uuid::new_v4(),
                timestamp: time::OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: NodeId::Cloud,
                target: NodeId::Cloud,
            },
            payload: MessagePayload::CloudCommand(CloudCommand::SendAnalyse {
                windows,
                reason: rec.reason,
                confidence: 0.8,
            }),
        }
    }

    async fn get_device_id_from_mac(&self, _mac_addr: [u8; 6]) -> Result<Id, MessageError> {
        Ok(1)
    }
}

impl MessageHandler for AnalyticsHandler {
    fn handle_message(&mut self, message: Message) -> Result<Option<Message>, MessageError> {
        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                match &message.payload {
                    MessagePayload::DeviceReport(device_report) => {
                        self.handle_device_report(&message, device_report).await
                    }
                    _ => Ok(None),
                }
            })
        })
    }

    fn supported_payloads(&self) -> Vec<PayloadType> {
        vec![PayloadType::DeviceReport]
    }

    fn node_id(&self) -> NodeId {
        NodeId::Cloud
    }

    fn name(&self) -> &'static str {
        "AnalyticsHandler"
    }
}

#[derive(Debug, Clone)]
struct Recommendation {
    device_id: Id,
    position: u8,
    reason: String,
}
