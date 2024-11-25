use std::collections::BTreeMap;
use std::sync::Arc;

use lumisync_api::adapter::AdapterManager;
use lumisync_api::handler::{MessageError, MessageHandler, PayloadType};
use lumisync_api::message::*;
use lumisync_api::uuid::{RandomUuidGenerator, UuidGenerator};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

use crate::configs::Storage;

pub struct CommandDispatcher {
    /// Transport adapter manager
    adapter_manager: Arc<Mutex<AdapterManager>>,
    /// Device routing table (device_id -> edge_id)
    device_routing: Arc<RwLock<BTreeMap<Id, u8>>>,
    /// UUID generator
    uuid_generator: RandomUuidGenerator,
    /// Storage for auditing
    _storage: Option<Arc<Storage>>,
}

impl CommandDispatcher {
    pub fn new(
        adapter_manager: Arc<Mutex<AdapterManager>>,
        device_routing: BTreeMap<Id, u8>,
        storage: Option<Arc<Storage>>,
    ) -> Self {
        Self {
            adapter_manager,
            device_routing: Arc::new(RwLock::new(device_routing)),
            uuid_generator: RandomUuidGenerator,
            _storage: storage,
        }
    }

    /// Update device routing table
    pub async fn update_device_routing(&self, device_id: Id, edge_id: u8) {
        let mut routing = self.device_routing.write().await;
        routing.insert(device_id, edge_id);
        debug!("Device {} routed to Edge {}", device_id, edge_id);
    }

    /// Remove device routing
    pub async fn remove_device_routing(&self, device_id: Id) {
        let mut routing = self.device_routing.write().await;
        if routing.remove(&device_id).is_some() {
            debug!("Removed routing for device {}", device_id);
        }
    }

    /// Handle Cloud commands
    async fn handle_cloud_command(
        &mut self,
        message: &Message,
        command: &CloudCommand,
    ) -> Result<Option<Message>, MessageError> {
        match command {
            CloudCommand::ConfigureRegion { plan } => {
                self.handle_region_configuration(message, plan).await
            }
            CloudCommand::ConfigureWindow { device_id, plan } => {
                self.handle_window_configuration(message, *device_id, plan)
                    .await
            }
            CloudCommand::ControlDevices { commands } => {
                self.handle_device_control(message, commands).await
            }
            CloudCommand::SendAnalyse {
                windows,
                reason,
                confidence,
            } => {
                self.handle_analysis_suggestions(message, windows, reason, *confidence)
                    .await
            }
        }
    }

    /// Handle region configuration command
    async fn handle_region_configuration(
        &mut self,
        message: &Message,
        plan: &[RegionSettingData],
    ) -> Result<Option<Message>, MessageError> {
        info!(
            "Processing region configuration command, settings count: {}",
            plan.len()
        );

        // Get all Edge nodes in the region
        let routing = self.device_routing.read().await;
        let target_edges: std::collections::HashSet<u8> = routing.values().cloned().collect();

        // Send configuration to each Edge
        let mut adapter_manager = self.adapter_manager.lock().await;
        for edge_id in target_edges {
            let edge_message = Message {
                header: MessageHeader {
                    id: self.uuid_generator.generate(),
                    timestamp: time::OffsetDateTime::now_utc(),
                    priority: message.header.priority,
                    source: NodeId::Cloud,
                    target: NodeId::Edge(edge_id),
                },
                payload: MessagePayload::CloudCommand(CloudCommand::ConfigureRegion {
                    plan: plan.to_vec(),
                }),
            };

            if let Err(e) = adapter_manager.send_to(NodeId::Edge(edge_id), &edge_message) {
                debug!(
                    "Failed to send region configuration to Edge {}: {}",
                    edge_id, e
                );
            } else {
                debug!("Region configuration sent to Edge {}", edge_id);
            }
        }

        Ok(Some(self.create_ack_message(
            message,
            "Region configuration command dispatched",
        )))
    }

    /// Handle window configuration command
    async fn handle_window_configuration(
        &mut self,
        message: &Message,
        device_id: Id,
        plan: &[WindowSettingData],
    ) -> Result<Option<Message>, MessageError> {
        info!(
            "Processing window configuration command for device {}",
            device_id
        );

        // Find Edge corresponding to device
        let routing = self.device_routing.read().await;
        let edge_id = routing.get(&device_id).ok_or_else(|| {
            MessageError::InvalidMessage(format!(
                "No corresponding Edge node found for device {}",
                device_id
            ))
        })?;
        let edge_id = *edge_id;
        drop(routing);

        // Send configuration command to Edge
        let edge_message = Message {
            header: MessageHeader {
                id: self.uuid_generator.generate(),
                timestamp: time::OffsetDateTime::now_utc(),
                priority: message.header.priority,
                source: NodeId::Cloud,
                target: NodeId::Edge(edge_id),
            },
            payload: MessagePayload::CloudCommand(CloudCommand::ConfigureWindow {
                device_id,
                plan: plan.to_vec(),
            }),
        };

        let mut adapter_manager = self.adapter_manager.lock().await;
        adapter_manager
            .send_to(NodeId::Edge(edge_id), &edge_message)
            .map_err(|e| {
                MessageError::TransportError(format!("Failed to send window configuration: {}", e))
            })?;

        debug!(
            "Window configuration sent to Edge {} (device {})",
            edge_id, device_id
        );

        Ok(Some(self.create_ack_message(
            message,
            &format!("Window configuration command for device {} sent", device_id),
        )))
    }

    /// Handle device control command
    async fn handle_device_control(
        &mut self,
        message: &Message,
        commands: &BTreeMap<Id, Command>,
    ) -> Result<Option<Message>, MessageError> {
        info!(
            "Processing device control command, device count: {}",
            commands.len()
        );

        let routing = self.device_routing.read().await;
        let mut edge_commands: BTreeMap<u8, BTreeMap<Id, Command>> = BTreeMap::new();

        // Group commands by Edge
        for (device_id, command) in commands {
            if let Some(edge_id) = routing.get(device_id) {
                edge_commands
                    .entry(*edge_id)
                    .or_insert_with(BTreeMap::new)
                    .insert(*device_id, command.clone());
            } else {
                warn!(
                    "No corresponding Edge node found for device {}, skipping command",
                    device_id
                );
            }
        }
        drop(routing);

        // Send commands to respective Edges
        let mut adapter_manager = self.adapter_manager.lock().await;
        let mut successful_devices: Vec<Id> = Vec::new();
        let mut failed_devices: Vec<Id> = Vec::new();

        for (edge_id, edge_commands_map) in edge_commands {
            let edge_message = Message {
                header: MessageHeader {
                    id: self.uuid_generator.generate(),
                    timestamp: time::OffsetDateTime::now_utc(),
                    priority: message.header.priority,
                    source: NodeId::Cloud,
                    target: NodeId::Edge(edge_id),
                },
                payload: MessagePayload::CloudCommand(CloudCommand::ControlDevices {
                    commands: edge_commands_map.clone(),
                }),
            };

            match adapter_manager.send_to(NodeId::Edge(edge_id), &edge_message) {
                Ok(_) => {
                    debug!(
                        "Device control command sent to Edge {}, device count: {}",
                        edge_id,
                        edge_commands_map.len()
                    );
                    successful_devices.extend(edge_commands_map.keys());
                }
                Err(e) => {
                    debug!(
                        "Failed to send device control command to Edge {}: {}",
                        edge_id, e
                    );
                    failed_devices.extend(edge_commands_map.keys());
                }
            }
        }

        let response_msg = if failed_devices.is_empty() {
            format!(
                "Control commands for all {} devices sent",
                successful_devices.len()
            )
        } else {
            format!(
                "{} device commands sent successfully, {} failed",
                successful_devices.len(),
                failed_devices.len()
            )
        };

        Ok(Some(self.create_ack_message(message, &response_msg)))
    }

    /// Handle analysis suggestions
    async fn handle_analysis_suggestions(
        &mut self,
        message: &Message,
        windows: &BTreeMap<Id, WindowData>,
        reason: &str,
        confidence: f32,
    ) -> Result<Option<Message>, MessageError> {
        info!(
            "Processing analysis suggestions, devices involved: {}, confidence: {:.2}",
            windows.len(),
            confidence
        );

        let routing = self.device_routing.read().await;
        let mut edge_suggestions: BTreeMap<u8, BTreeMap<Id, WindowData>> = BTreeMap::new();

        // Group suggestions by Edge
        for (device_id, window_data) in windows {
            if let Some(edge_id) = routing.get(device_id) {
                edge_suggestions
                    .entry(*edge_id)
                    .or_insert_with(BTreeMap::new)
                    .insert(*device_id, window_data.clone());
            } else {
                warn!(
                    "No corresponding Edge node found for device {}, skipping suggestion",
                    device_id
                );
            }
        }
        drop(routing);

        // Send suggestions to respective Edges
        let mut adapter_manager = self.adapter_manager.lock().await;
        for (edge_id, edge_windows) in edge_suggestions {
            let edge_message = Message {
                header: MessageHeader {
                    id: self.uuid_generator.generate(),
                    timestamp: time::OffsetDateTime::now_utc(),
                    priority: Priority::Regular, // Suggestions are usually not urgent
                    source: NodeId::Cloud,
                    target: NodeId::Edge(edge_id),
                },
                payload: MessagePayload::CloudCommand(CloudCommand::SendAnalyse {
                    windows: edge_windows.clone(),
                    reason: reason.to_string(),
                    confidence,
                }),
            };

            if let Err(e) = adapter_manager.send_to(NodeId::Edge(edge_id), &edge_message) {
                debug!(
                    "Failed to send analysis suggestions to Edge {}: {}",
                    edge_id, e
                );
            } else {
                debug!(
                    "Analysis suggestions sent to Edge {}, device count: {}",
                    edge_id,
                    edge_windows.len()
                );
            }
        }

        Ok(Some(self.create_ack_message(
            message,
            &format!("Analysis suggestions sent to {} devices", windows.len()),
        )))
    }

    /// Create acknowledgment response message
    fn create_ack_message(&self, original: &Message, status: &str) -> Message {
        Message {
            header: MessageHeader {
                id: self.uuid_generator.generate(),
                timestamp: time::OffsetDateTime::now_utc(),
                priority: original.header.priority,
                source: NodeId::Cloud,
                target: original.header.source,
            },
            payload: MessagePayload::Acknowledge(AckPayload {
                original_msg_id: original.header.id,
                status: status.to_string(),
                details: None,
            }),
        }
    }

    /// Get dispatch statistics
    pub async fn get_dispatch_stats(&self) -> DispatchStats {
        let routing = self.device_routing.read().await;

        DispatchStats {
            total_devices: routing.len(),
            active_edges: routing
                .values()
                .collect::<std::collections::HashSet<_>>()
                .len(),
        }
    }
}

impl MessageHandler for CommandDispatcher {
    fn handle_message(&mut self, message: Message) -> Result<Option<Message>, MessageError> {
        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                match &message.payload {
                    MessagePayload::CloudCommand(command) => {
                        self.handle_cloud_command(&message, command).await
                    }
                    _ => Ok(None),
                }
            })
        })
    }

    fn supported_payloads(&self) -> Vec<PayloadType> {
        vec![PayloadType::CloudCommand]
    }

    fn node_id(&self) -> NodeId {
        NodeId::Cloud
    }

    fn name(&self) -> &'static str {
        "CommandDispatcher"
    }
}

#[derive(Debug, Clone)]
pub struct DispatchStats {
    pub total_devices: usize,
    pub active_edges: usize,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use lumisync_api::adapter::AdapterManager;

    use super::*;

    #[tokio::test]
    async fn test_device_routing_update() {
        let adapter_manager = Arc::new(Mutex::new(AdapterManager::new()));
        let device_routing = BTreeMap::new();

        let dispatcher = CommandDispatcher::new(adapter_manager, device_routing, None);

        dispatcher.update_device_routing(1, 3).await;

        let routing = dispatcher.device_routing.read().await;
        assert_eq!(routing.get(&1), Some(&3));
    }

    #[tokio::test]
    async fn test_device_routing_removal() {
        let adapter_manager = Arc::new(Mutex::new(AdapterManager::new()));
        let mut device_routing = BTreeMap::new();
        device_routing.insert(1, 2u8);
        device_routing.insert(2, 2u8);

        let dispatcher = CommandDispatcher::new(adapter_manager, device_routing, None);

        // Verify initial routing
        {
            let routing = dispatcher.device_routing.read().await;
            assert_eq!(routing.get(&1), Some(&2));
            assert_eq!(routing.get(&2), Some(&2));
        }

        // Remove device routing
        dispatcher.remove_device_routing(1).await;

        // Verify removal
        {
            let routing = dispatcher.device_routing.read().await;
            assert_eq!(routing.get(&1), None);
            assert_eq!(routing.get(&2), Some(&2)); // Other routing should remain
        }
    }

    #[tokio::test]
    async fn test_dispatch_stats() {
        let adapter_manager = Arc::new(Mutex::new(AdapterManager::new()));
        let mut device_routing = BTreeMap::new();
        device_routing.insert(1, 1u8);
        device_routing.insert(2, 1u8);
        device_routing.insert(3, 2u8);

        let dispatcher = CommandDispatcher::new(adapter_manager, device_routing, None);

        let stats = dispatcher.get_dispatch_stats().await;
        assert_eq!(stats.total_devices, 3);
        assert_eq!(stats.active_edges, 2); // Edge 1 and Edge 2
    }
}
