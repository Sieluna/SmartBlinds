use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use lumisync_api::message::{Message, MessagePayload, NodeId, TimeSyncPayload};
use lumisync_api::time::{SyncConfig, SyncStatus, TimeProvider, TimeSyncService};
use lumisync_api::uuid::DeviceBasedUuidGenerator;
use time::OffsetDateTime;

use crate::error::{Error, Result};

pub struct EmbeddedTimeProvider {
    boot_time: embassy_time::Instant,
}

impl EmbeddedTimeProvider {
    pub fn new() -> Self {
        Self {
            boot_time: embassy_time::Instant::now(),
        }
    }
}

impl TimeProvider for EmbeddedTimeProvider {
    fn monotonic_time_ms(&self) -> u64 {
        embassy_time::Instant::now()
            .duration_since(self.boot_time)
            .as_millis()
    }

    fn wall_clock_time(&self) -> Option<OffsetDateTime> {
        None
    }

    fn has_authoritative_time(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
pub struct DeviceSyncInfo {
    pub mac: [u8; 6],
    pub last_sync_time: Option<u64>,
    pub sync_status: DeviceSyncStatus,
    pub precision_requirement_ms: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceSyncStatus {
    Unsynced,
    Syncing,
    Synced,
    Failed,
}

impl From<SyncStatus> for DeviceSyncStatus {
    fn from(status: SyncStatus) -> Self {
        match status {
            SyncStatus::Unsynced => DeviceSyncStatus::Unsynced,
            SyncStatus::Synced => DeviceSyncStatus::Synced,
            SyncStatus::Failed { .. } => DeviceSyncStatus::Failed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeviceSyncStats {
    pub total_devices: usize,
    pub synced_devices: usize,
    pub unsynced_devices: usize,
    pub stale_devices: usize,
    pub edge_sync_status: DeviceSyncStatus,
    pub last_broadcast_time: Option<u64>,
}

impl DeviceSyncStats {
    pub fn device_sync_rate(&self) -> f64 {
        if self.total_devices == 0 {
            0.0
        } else {
            self.synced_devices as f64 / self.total_devices as f64
        }
    }

    pub fn needs_frequent_broadcast(&self) -> bool {
        self.device_sync_rate() < 0.8 || self.stale_devices > 0
    }
}

pub struct EdgeTimeSyncService {
    time_service: TimeSyncService<EmbeddedTimeProvider, DeviceBasedUuidGenerator>,
    edge_id: u8,
    last_broadcast_time: Option<u64>,
    device_broadcast_interval_ms: u64,
    connected_devices: BTreeMap<[u8; 6], DeviceSyncInfo>,
}

impl EdgeTimeSyncService {
    pub fn new(
        edge_id: u8,
        sync_config: SyncConfig,
        uuid_generator: DeviceBasedUuidGenerator,
    ) -> Self {
        let time_provider = EmbeddedTimeProvider::new();
        let time_service = TimeSyncService::new(
            time_provider,
            NodeId::Edge(edge_id),
            sync_config,
            uuid_generator,
        );

        Self {
            time_service,
            edge_id,
            last_broadcast_time: None,
            device_broadcast_interval_ms: 60000,
            connected_devices: BTreeMap::new(),
        }
    }

    pub fn needs_sync(&self) -> bool {
        self.time_service.needs_sync()
    }

    pub fn get_sync_status(&self) -> DeviceSyncStatus {
        self.time_service.get_sync_status().into()
    }

    pub fn get_network_time(&self) -> Result<OffsetDateTime> {
        let current_uptime = self.time_service.get_time_provider().monotonic_time_ms();
        self.time_service
            .get_network_time(current_uptime)
            .map_err(|_| Error::InvalidCommand)
    }

    pub fn create_sync_request(&mut self, target: NodeId) -> Result<Message> {
        self.time_service
            .create_sync_request(target)
            .map_err(|_| Error::InvalidCommand)
    }

    pub fn handle_sync_response(&mut self, response: &Message) -> Result<()> {
        self.time_service
            .handle_sync_response(response)
            .map_err(|_| Error::InvalidCommand)
    }

    pub fn handle_device_sync_request(&mut self, request: &Message) -> Result<Option<Message>> {
        if let NodeId::Device(device_mac) = request.header.source {
            if let MessagePayload::TimeSync(TimeSyncPayload::Request { precision_ms, .. }) =
                &request.payload
            {
                self.update_device_sync_info(device_mac, *precision_ms);

                match self.time_service.handle_sync_request(request) {
                    Ok(response) => return Ok(Some(response)),
                    Err(_) => return Err(Error::InvalidCommand),
                }
            }
        }

        Ok(None)
    }

    pub fn handle_status_query(&self, request: &Message) -> Result<Message> {
        self.time_service
            .handle_status_query(request)
            .map_err(|_| Error::InvalidCommand)
    }

    pub fn needs_device_broadcast(&self) -> bool {
        if let Some(last_broadcast) = self.last_broadcast_time {
            let current_time = self.time_service.get_time_provider().monotonic_time_ms();
            current_time.saturating_sub(last_broadcast) > self.device_broadcast_interval_ms
        } else {
            true
        }
    }

    pub fn create_device_broadcast(&mut self) -> Result<Vec<Message>> {
        let network_time = self.get_network_time()?;
        let mut messages = Vec::new();

        for (device_mac, _device_info) in &self.connected_devices {
            let broadcast_payload = TimeSyncPayload::Broadcast {
                timestamp: network_time,
                offset_ms: self.time_service.get_current_offset_ms(),
                accuracy_ms: self.get_sync_accuracy().unwrap_or(u16::MAX),
            };

            let broadcast_message = Message {
                header: lumisync_api::message::MessageHeader {
                    id: uuid::Uuid::new_v4(),
                    timestamp: network_time,
                    priority: lumisync_api::message::Priority::Regular,
                    source: NodeId::Edge(self.edge_id),
                    target: NodeId::Device(*device_mac),
                },
                payload: MessagePayload::TimeSync(broadcast_payload),
            };

            messages.push(broadcast_message);
        }

        self.last_broadcast_time = Some(self.time_service.get_time_provider().monotonic_time_ms());

        Ok(messages)
    }

    pub fn mark_broadcast_needed(&mut self) {
        self.last_broadcast_time = None;
    }

    pub fn add_connected_device(&mut self, device_mac: [u8; 6], precision_requirement_ms: u16) {
        let device_info = DeviceSyncInfo {
            mac: device_mac,
            last_sync_time: None,
            sync_status: DeviceSyncStatus::Unsynced,
            precision_requirement_ms,
        };

        self.connected_devices.insert(device_mac, device_info);
        self.mark_broadcast_needed();
    }

    pub fn remove_disconnected_device(&mut self, device_mac: [u8; 6]) {
        self.connected_devices.remove(&device_mac);
    }

    fn update_device_sync_info(&mut self, device_mac: [u8; 6], precision_ms: u16) {
        if let Some(device_info) = self.connected_devices.get_mut(&device_mac) {
            device_info.last_sync_time =
                Some(self.time_service.get_time_provider().monotonic_time_ms());
            device_info.sync_status = DeviceSyncStatus::Synced;
            device_info.precision_requirement_ms = precision_ms;
        } else {
            self.add_connected_device(device_mac, precision_ms);
        }
    }

    pub fn get_device_sync_status(&self, device_mac: [u8; 6]) -> Option<&DeviceSyncInfo> {
        self.connected_devices.get(&device_mac)
    }

    pub fn get_device_sync_stats(&self) -> DeviceSyncStats {
        let total_devices = self.connected_devices.len();
        let synced_devices = self
            .connected_devices
            .values()
            .filter(|info| info.sync_status == DeviceSyncStatus::Synced)
            .count();
        let unsynced_devices = total_devices - synced_devices;

        let current_time = self.time_service.get_time_provider().monotonic_time_ms();
        let stale_devices = self
            .connected_devices
            .values()
            .filter(|info| {
                if let Some(last_sync) = info.last_sync_time {
                    current_time.saturating_sub(last_sync) > 300_000
                } else {
                    true
                }
            })
            .count();

        DeviceSyncStats {
            total_devices,
            synced_devices,
            unsynced_devices,
            stale_devices,
            edge_sync_status: self.get_sync_status(),
            last_broadcast_time: self.last_broadcast_time,
        }
    }

    pub fn reset_sync(&mut self) {
        self.time_service.reset_sync();
        self.last_broadcast_time = None;

        for device_info in self.connected_devices.values_mut() {
            device_info.sync_status = DeviceSyncStatus::Unsynced;
            device_info.last_sync_time = None;
        }
    }

    pub fn get_sync_accuracy(&self) -> Option<u16> {
        let accuracy = self.time_service.get_current_accuracy();
        if accuracy == u16::MAX {
            None
        } else {
            Some(accuracy)
        }
    }

    pub fn get_current_offset_ms(&self) -> i64 {
        self.time_service.get_current_offset_ms()
    }

    pub fn edge_id(&self) -> u8 {
        self.edge_id
    }

    pub fn connected_device_count(&self) -> usize {
        self.connected_devices.len()
    }

    pub fn set_device_broadcast_interval(&mut self, interval_ms: u64) {
        self.device_broadcast_interval_ms = interval_ms;
    }

    pub fn time_sync_service(
        &self,
    ) -> &TimeSyncService<EmbeddedTimeProvider, DeviceBasedUuidGenerator> {
        &self.time_service
    }

    pub fn time_sync_service_mut(
        &mut self,
    ) -> &mut TimeSyncService<EmbeddedTimeProvider, DeviceBasedUuidGenerator> {
        &mut self.time_service
    }
}
