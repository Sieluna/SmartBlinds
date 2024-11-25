use alloc::collections::BTreeMap;

use lumisync_api::message::{Message, MessagePayload, NodeId, TimeSyncPayload};
use lumisync_api::time::{SyncConfig, SyncStatus, TimeSynchronizer};
use lumisync_api::uuid::{DeviceBasedUuidGenerator, UuidGenerator};

use crate::error::{Error, Result};

pub struct DeviceTimeSyncClient {
    device_mac: [u8; 6],
    connected_edge: Option<u8>,
    uuid_generator: DeviceBasedUuidGenerator,
    synchronizer: TimeSynchronizer,
    sequence_counter: u32,
    pending_requests: BTreeMap<u32, u64>,
    precision_requirement_ms: u16,
    accept_broadcast: bool,
    last_network_delay_ms: u32,
}

impl DeviceTimeSyncClient {
    pub fn new(
        device_mac: [u8; 6],
        sync_config: SyncConfig,
        precision_requirement_ms: u16,
        uuid_generator: DeviceBasedUuidGenerator,
    ) -> Self {
        Self {
            device_mac,
            connected_edge: None,
            uuid_generator,
            synchronizer: TimeSynchronizer::new(NodeId::Device(device_mac), sync_config),
            sequence_counter: 0,
            pending_requests: BTreeMap::new(),
            precision_requirement_ms,
            accept_broadcast: true,
            last_network_delay_ms: 20,
        }
    }

    pub fn device_mac(&self) -> [u8; 6] {
        self.device_mac
    }

    pub fn connect_to_edge(&mut self, edge_id: u8) {
        self.connected_edge = Some(edge_id);
        log::info!("Device {:?} connected to Edge {}", self.device_mac, edge_id);

        self.synchronizer.reset();
        self.pending_requests.clear();
    }

    pub fn disconnect_from_edge(&mut self) {
        if let Some(edge_id) = self.connected_edge.take() {
            log::info!(
                "Device {:?} disconnected from Edge {}",
                self.device_mac,
                edge_id
            );
            self.synchronizer.reset();
            self.pending_requests.clear();
        }
    }

    pub fn needs_active_sync(&self) -> bool {
        if self.connected_edge.is_none() {
            return false;
        }

        let current_uptime = embassy_time::Instant::now().as_millis();
        self.synchronizer.needs_sync(current_uptime)
    }

    pub fn create_edge_sync_request(&mut self) -> Result<Option<Message>> {
        if let Some(edge_id) = self.connected_edge {
            let current_uptime = embassy_time::Instant::now().as_millis();

            self.cleanup_expired_requests(current_uptime);

            self.sequence_counter = self.sequence_counter.wrapping_add(1);
            let sequence = self.sequence_counter;

            self.pending_requests.insert(sequence, current_uptime);

            let send_time = if matches!(self.synchronizer.status, SyncStatus::Synced) {
                if let Ok(network_time_ms) =
                    self.synchronizer.uptime_to_network_time(current_uptime)
                {
                    let timestamp_secs = (network_time_ms / 1000) as i64;
                    let timestamp_nanos = ((network_time_ms % 1000) as u32) * 1_000_000;

                    time::OffsetDateTime::from_unix_timestamp(timestamp_secs)
                        .and_then(|dt| dt.replace_nanosecond(timestamp_nanos))
                        .ok()
                } else {
                    None
                }
            } else {
                None
            };

            let sync_request = Message {
                header: lumisync_api::message::MessageHeader {
                    id: self.uuid_generator.generate(),
                    timestamp: time::OffsetDateTime::UNIX_EPOCH,
                    priority: lumisync_api::message::Priority::Regular,
                    source: NodeId::Device(self.device_mac),
                    target: NodeId::Edge(edge_id),
                },
                payload: MessagePayload::TimeSync(TimeSyncPayload::Request {
                    sequence,
                    send_time,
                    precision_ms: self.precision_requirement_ms,
                }),
            };

            log::debug!("Created sync request {} to Edge {}", sequence, edge_id);
            Ok(Some(sync_request))
        } else {
            Ok(None)
        }
    }

    pub fn handle_edge_sync_response(&mut self, response: &Message) -> Result<()> {
        if let NodeId::Edge(edge_id) = response.header.source {
            if Some(edge_id) != self.connected_edge {
                return Err(Error::InvalidCommand);
            }

            if let MessagePayload::TimeSync(TimeSyncPayload::Response {
                request_sequence,
                response_send_time,
                estimated_delay_ms,
                ..
            }) = &response.payload
            {
                if let Some(request_uptime) = self.pending_requests.remove(request_sequence) {
                    let current_uptime = embassy_time::Instant::now().as_millis();

                    let response_time_ms = response_send_time.unix_timestamp() as u64 * 1000
                        + (response_send_time.nanosecond() / 1_000_000) as u64;

                    match self.synchronizer.handle_sync_response(
                        request_uptime,
                        response_time_ms,
                        current_uptime,
                    ) {
                        Ok(()) => {
                            self.last_network_delay_ms = *estimated_delay_ms;
                            log::debug!(
                                "Synchronized with Edge {}, offset: {}ms",
                                edge_id,
                                self.synchronizer.get_current_offset_ms()
                            );
                            Ok(())
                        }
                        Err(sync_error) => {
                            log::warn!("Sync failed: {}", sync_error);
                            Err(Error::InvalidCommand)
                        }
                    }
                } else {
                    log::warn!(
                        "Received response for unknown request sequence: {}",
                        request_sequence
                    );
                    Err(Error::InvalidCommand)
                }
            } else {
                Err(Error::InvalidCommand)
            }
        } else {
            Err(Error::InvalidCommand)
        }
    }

    pub fn handle_edge_broadcast(&mut self, broadcast: &Message) -> Result<()> {
        if !self.accept_broadcast {
            return Ok(());
        }

        if let NodeId::Edge(edge_id) = broadcast.header.source {
            if Some(edge_id) == self.connected_edge {
                if let MessagePayload::TimeSync(TimeSyncPayload::Broadcast {
                    accuracy_ms, ..
                }) = &broadcast.payload
                {
                    let current_uptime = embassy_time::Instant::now().as_millis();
                    self.synchronizer.update_status(current_uptime);

                    log::debug!(
                        "Received time broadcast from Edge {}, accuracy: {}ms",
                        edge_id,
                        accuracy_ms
                    );
                    Ok(())
                } else {
                    Err(Error::InvalidCommand)
                }
            } else {
                Err(Error::InvalidCommand)
            }
        } else {
            Err(Error::InvalidCommand)
        }
    }

    pub fn get_network_time(&self) -> Result<time::OffsetDateTime> {
        let current_uptime = embassy_time::Instant::now().as_millis();

        match self.synchronizer.uptime_to_network_time(current_uptime) {
            Ok(network_time_ms) => {
                let timestamp_secs = (network_time_ms / 1000) as i64;
                let timestamp_nanos = ((network_time_ms % 1000) as u32) * 1_000_000;

                time::OffsetDateTime::from_unix_timestamp(timestamp_secs)
                    .and_then(|dt| dt.replace_nanosecond(timestamp_nanos))
                    .map_err(|_| Error::InvalidCommand)
            }
            Err(_) => Err(Error::InvalidCommand),
        }
    }

    pub fn get_local_accuracy(&self) -> Option<u16> {
        if self.connected_edge.is_some() && matches!(self.synchronizer.status, SyncStatus::Synced) {
            let network_accuracy =
                self.last_network_delay_ms as u16 + self.precision_requirement_ms;
            Some(network_accuracy.min(u16::MAX))
        } else {
            None
        }
    }

    pub fn get_sync_stats(&self) -> DeviceSyncStats {
        let current_time = embassy_time::Instant::now().as_millis();

        let time_since_last_sync = self
            .synchronizer
            .last_sync_uptime
            .map(|last_sync| current_time.saturating_sub(last_sync));

        DeviceSyncStats {
            device_mac: self.device_mac,
            connected_edge: self.connected_edge,
            time_since_last_sync,
            precision_requirement_ms: self.precision_requirement_ms,
            estimated_accuracy_ms: self.get_local_accuracy(),
            sync_status: self.synchronizer.get_status(),
            current_offset_ms: self.synchronizer.get_current_offset_ms(),
            last_network_delay_ms: self.last_network_delay_ms,
            pending_requests: self.pending_requests.len(),
        }
    }

    pub fn set_precision_requirement(&mut self, precision_ms: u16) {
        self.precision_requirement_ms = precision_ms;
        log::info!("Updated precision requirement to {}ms", precision_ms);
    }

    pub fn set_accept_broadcast(&mut self, accept: bool) {
        self.accept_broadcast = accept;
        log::info!("Set accept broadcast to {}", accept);
    }

    pub fn force_resync(&mut self) {
        self.synchronizer.reset();
        self.pending_requests.clear();
        log::info!("Forced sync reset for device {:?}", self.device_mac);
    }

    pub fn check_sync_health(&self) -> SyncHealthStatus {
        let current_time = embassy_time::Instant::now().as_millis();

        if self.connected_edge.is_none() {
            return SyncHealthStatus::Unsynced;
        }

        match self.synchronizer.status {
            SyncStatus::Synced => {
                if let Some(last_sync) = self.synchronizer.last_sync_uptime {
                    let time_since_sync = current_time.saturating_sub(last_sync);

                    if time_since_sync > 600_000 {
                        SyncHealthStatus::Critical
                    } else if time_since_sync > 300_000 {
                        SyncHealthStatus::Stale
                    } else {
                        SyncHealthStatus::Healthy
                    }
                } else {
                    SyncHealthStatus::Unsynced
                }
            }
            SyncStatus::Failed { .. } => SyncHealthStatus::Failed,
            SyncStatus::Unsynced => SyncHealthStatus::Unsynced,
        }
    }

    fn cleanup_expired_requests(&mut self, current_uptime: u64) {
        let timeout_ms = 10000;
        self.pending_requests
            .retain(|_, req_uptime| current_uptime.saturating_sub(*req_uptime) < timeout_ms);
    }

    pub fn get_synchronizer(&self) -> &TimeSynchronizer {
        &self.synchronizer
    }

    pub fn get_synchronizer_mut(&mut self) -> &mut TimeSynchronizer {
        &mut self.synchronizer
    }
}

#[derive(Debug, Clone)]
pub struct DeviceSyncStats {
    pub device_mac: [u8; 6],
    pub connected_edge: Option<u8>,
    pub time_since_last_sync: Option<u64>,
    pub precision_requirement_ms: u16,
    pub estimated_accuracy_ms: Option<u16>,
    pub sync_status: SyncStatus,
    pub current_offset_ms: i64,
    pub last_network_delay_ms: u32,
    pub pending_requests: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncHealthStatus {
    Healthy,
    Unsynced,
    Failed,
    Stale,
    Critical,
}

impl DeviceSyncStats {
    pub fn needs_urgent_sync(&self) -> bool {
        !matches!(self.sync_status, SyncStatus::Synced)
            || self.connected_edge.is_none()
            || self.time_since_last_sync.map_or(true, |t| t > 600_000)
    }

    pub fn sync_quality_score(&self) -> u8 {
        if self.connected_edge.is_none() || !matches!(self.sync_status, SyncStatus::Synced) {
            return 0;
        }

        if let Some(time_since_sync) = self.time_since_last_sync {
            let minutes_since_sync = time_since_sync / 60_000;

            let base_score: u8 = match minutes_since_sync {
                0..=2 => 100,
                3..=5 => 90,
                6..=10 => 70,
                11..=15 => 50,
                _ => 20,
            };

            let delay_penalty = (self.last_network_delay_ms / 10) as u8;
            base_score.saturating_sub(delay_penalty)
        } else {
            0
        }
    }
}
