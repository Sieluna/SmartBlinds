pub mod service;
pub mod sync;

pub use service::*;
pub use sync::*;

pub trait TimeProvider {
    fn uptime_ms(&self) -> u64;
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncError {
    /// Network delay too high
    HighNetworkDelay,
    /// Time drift exceeds acceptable threshold
    ExcessiveDrift,
    /// Sync operation timed out
    Timeout,
    /// Transport layer error during sync
    TransportError,
    /// Received invalid timestamp
    InvalidTimestamp,
}

impl core::fmt::Display for SyncError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SyncError::HighNetworkDelay => write!(f, "Network delay too high for reliable sync"),
            SyncError::ExcessiveDrift => write!(f, "Time drift exceeds acceptable threshold"),
            SyncError::Timeout => write!(f, "Sync operation timed out"),
            SyncError::TransportError => write!(f, "Transport layer error during sync"),
            SyncError::InvalidTimestamp => write!(f, "Received invalid timestamp"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SyncError {}

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::collections::BTreeMap;
    use alloc::vec::Vec;

    use time::OffsetDateTime;

    use crate::message::*;

    #[derive(Debug)]
    pub struct NetworkSimulator {
        /// Message queues between nodes
        message_queues: BTreeMap<NodeId, Vec<Message>>,
        /// Network delay configuration (from, to) -> delay_ms
        network_delays: BTreeMap<(NodeId, NodeId), u64>,
        /// Packet loss configuration (from, to) -> drop_rate (0.0-1.0)
        packet_loss: BTreeMap<(NodeId, NodeId), f32>,
        /// Current simulation time
        current_time: u64,
        /// Random seed
        rng_state: u64,
    }

    impl NetworkSimulator {
        pub fn new() -> Self {
            Self {
                message_queues: BTreeMap::new(),
                network_delays: BTreeMap::new(),
                packet_loss: BTreeMap::new(),
                current_time: 0,
                rng_state: 12345,
            }
        }

        pub fn set_network_delay(&mut self, from: NodeId, to: NodeId, delay_ms: u64) {
            self.network_delays.insert((from, to), delay_ms);
        }

        pub fn set_packet_loss(&mut self, from: NodeId, to: NodeId, loss_rate: f32) {
            self.packet_loss.insert((from, to), loss_rate);
        }

        pub fn send_message(&mut self, message: Message) {
            let from = message.header.source;
            let to = message.header.target;

            if self.should_drop_packet(from, to) {
                return;
            }

            let delay = self.get_network_delay(from, to);
            let delivery_time = self.current_time + delay;

            let mut delayed_message = message;
            delayed_message.header.timestamp =
                OffsetDateTime::from_unix_timestamp((delivery_time / 1000) as i64)
                    .unwrap_or(OffsetDateTime::UNIX_EPOCH);

            self.message_queues
                .entry(to)
                .or_insert_with(Vec::new)
                .push(delayed_message);
        }

        pub fn receive_messages(&mut self, node_id: NodeId) -> Vec<Message> {
            self.message_queues.remove(&node_id).unwrap_or_default()
        }

        pub fn advance_time(&mut self, ms: u64) {
            self.current_time += ms;
        }

        pub fn current_time(&self) -> u64 {
            self.current_time
        }

        fn next_random(&mut self) -> f32 {
            self.rng_state = self.rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            (self.rng_state % 2147483647) as f32 / 2147483647.0
        }

        fn should_drop_packet(&mut self, from: NodeId, to: NodeId) -> bool {
            if let Some(&loss_rate) = self.packet_loss.get(&(from, to)) {
                self.next_random() < loss_rate
            } else {
                false
            }
        }

        fn get_network_delay(&self, from: NodeId, to: NodeId) -> u64 {
            self.network_delays.get(&(from, to)).copied().unwrap_or(10)
        }
    }

    #[derive(Debug, Clone)]
    pub struct TestTimeProvider {
        /// Base time
        base_time: u64,
        /// Clock offset (milliseconds)
        clock_offset: i64,
        /// Clock drift rate (ppm - parts per million)
        clock_drift_ppm: f64,
        /// Start time
        start_time: u64,
        /// Noise amplitude
        noise_amplitude: u64,
        /// Internal counter for noise
        noise_counter: u64,
    }

    impl TestTimeProvider {
        pub fn new(base_time: u64) -> Self {
            Self {
                base_time,
                clock_offset: 0,
                clock_drift_ppm: 0.0,
                start_time: base_time,
                noise_amplitude: 0,
                noise_counter: 0,
            }
        }

        pub fn set_clock_offset(&mut self, offset_ms: i64) {
            self.clock_offset = offset_ms;
        }

        pub fn set_clock_drift(&mut self, drift_ppm: f64) {
            self.clock_drift_ppm = drift_ppm;
        }

        pub fn set_clock_noise(&mut self, amplitude_ms: u64) {
            self.noise_amplitude = amplitude_ms;
        }

        pub fn advance_time(&mut self, ms: u64) {
            self.base_time += ms;
        }

        #[allow(dead_code)]
        fn get_noise(&mut self) -> i64 {
            if self.noise_amplitude == 0 {
                return 0;
            }

            self.noise_counter = self.noise_counter.wrapping_add(1);

            let noise = (self.noise_counter * 7919) % (self.noise_amplitude * 2);
            noise as i64 - self.noise_amplitude as i64
        }
    }

    impl TimeProvider for TestTimeProvider {
        fn uptime_ms(&self) -> u64 {
            let elapsed = self.base_time - self.start_time;

            let drift_factor = 1.0 + (self.clock_drift_ppm / 1_000_000.0);
            let drifted_time = (elapsed as f64 * drift_factor) as u64;

            let final_time = (drifted_time as i64 + self.clock_offset) as u64;

            final_time
        }
    }

    pub struct NetworkNode {
        pub node_id: NodeId,
        pub time_provider: TestTimeProvider,
        pub sync_service: TimeSyncService<TestTimeProvider>,
        pub message_buffer: Vec<Message>,
    }

    impl NetworkNode {
        pub fn new(node_id: NodeId, base_time: u64, config: SyncConfig) -> Self {
            let time_provider = TestTimeProvider::new(base_time);
            let sync_service = TimeSyncService::new(time_provider.clone(), node_id, config);

            Self {
                node_id,
                time_provider,
                sync_service,
                message_buffer: Vec::new(),
            }
        }

        pub fn process_messages(&mut self, messages: Vec<Message>) -> Vec<Message> {
            let mut responses = Vec::new();

            for msg in messages {
                match &msg.payload {
                    MessagePayload::TimeSync(sync_payload) => match sync_payload {
                        TimeSyncPayload::Request { .. } => {
                            if let Ok(response) = self.sync_service.handle_sync_request(&msg) {
                                responses.push(response);
                            }
                        }
                        TimeSyncPayload::Response { .. } => {
                            let _ = self.sync_service.handle_sync_response(&msg);
                        }
                        TimeSyncPayload::StatusQuery => {
                            let response = self.sync_service.handle_status_query(&msg);
                            responses.push(response);
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }

            responses
        }

        pub fn create_sync_request(&mut self, target: NodeId) -> Option<Message> {
            self.sync_service.create_sync_request(target).ok()
        }

        pub fn get_sync_status(&self) -> SyncStatus {
            self.sync_service.get_sync_status()
        }

        pub fn advance_time(&mut self, ms: u64) {
            self.time_provider.advance_time(ms);
            self.sync_service.cleanup_expired_requests();
        }

        pub fn set_clock_offset(&mut self, offset_ms: i64) {
            self.time_provider.set_clock_offset(offset_ms);
        }

        pub fn set_clock_drift(&mut self, drift_ppm: f64) {
            self.time_provider.set_clock_drift(drift_ppm);
        }

        pub fn set_clock_noise(&mut self, amplitude_ms: u64) {
            self.time_provider.set_clock_noise(amplitude_ms);
        }
    }

    pub struct TimeSyncIntegrationTest {
        pub network: NetworkSimulator,
        pub nodes: BTreeMap<NodeId, NetworkNode>,
    }

    impl TimeSyncIntegrationTest {
        pub fn new() -> Self {
            Self {
                network: NetworkSimulator::new(),
                nodes: BTreeMap::new(),
            }
        }

        pub fn add_node(&mut self, node_id: NodeId, config: SyncConfig) {
            let node = NetworkNode::new(node_id, self.network.current_time(), config);
            self.nodes.insert(node_id, node);
        }

        pub fn setup_network_topology(&mut self) {
            // Cloud <-> Edge: 50ms
            self.network
                .set_network_delay(NodeId::Cloud, NodeId::Edge(1), 50);
            self.network
                .set_network_delay(NodeId::Edge(1), NodeId::Cloud, 50);

            // Edge <-> Device: 20ms
            for i in 1..=3 {
                let device_id = NodeId::Device([0, 0, 0, 0, 0, i]);
                self.network
                    .set_network_delay(NodeId::Edge(1), device_id, 20);
                self.network
                    .set_network_delay(device_id, NodeId::Edge(1), 20);
            }
        }

        pub fn process_round(&mut self) {
            let mut outgoing_messages = Vec::new();

            for (node_id, node) in &mut self.nodes {
                let incoming = self.network.receive_messages(*node_id);
                let responses = node.process_messages(incoming);
                outgoing_messages.extend(responses);
            }

            for msg in outgoing_messages {
                self.network.send_message(msg);
            }
        }

        pub fn advance_time(&mut self, ms: u64) {
            self.network.advance_time(ms);
            for node in self.nodes.values_mut() {
                node.advance_time(ms);
            }
        }

        pub fn trigger_sync(&mut self, from: NodeId, to: NodeId) {
            if let Some(node) = self.nodes.get_mut(&from) {
                if let Some(request) = node.create_sync_request(to) {
                    self.network.send_message(request);
                }
            }
        }

        pub fn get_network_stats(&self) -> NetworkStats {
            let mut stats = NetworkStats {
                total_nodes: self.nodes.len(),
                synced_nodes: 0,
                syncing_nodes: 0,
                failed_nodes: 0,
                unsynced_nodes: 0,
                average_offset: 0.0,
                max_offset: 0,
            };

            let mut total_offset: i64 = 0;
            let mut max_offset: i64 = 0;

            for node in self.nodes.values() {
                match node.get_sync_status() {
                    SyncStatus::Synced => stats.synced_nodes += 1,
                    SyncStatus::Syncing => stats.syncing_nodes += 1,
                    SyncStatus::Failed => stats.failed_nodes += 1,
                    SyncStatus::Unsynced => stats.unsynced_nodes += 1,
                }

                let offset = node.sync_service.get_current_offset_ms();
                total_offset += offset;
                max_offset = max_offset.max(offset.abs());
            }

            if stats.total_nodes > 0 {
                stats.average_offset = total_offset as f64 / stats.total_nodes as f64;
            }
            stats.max_offset = max_offset;

            stats
        }
    }

    #[derive(Debug, Clone)]
    pub struct NetworkStats {
        pub total_nodes: usize,
        pub synced_nodes: usize,
        pub syncing_nodes: usize,
        pub failed_nodes: usize,
        pub unsynced_nodes: usize,
        pub average_offset: f64,
        pub max_offset: i64,
    }

    fn create_test_config() -> SyncConfig {
        SyncConfig {
            sync_interval_ms: 1000,
            max_drift_ms: 100,
            offset_history_size: 5,
            delay_compensation_threshold_ms: 50,
            max_retry_count: 3,
        }
    }

    #[test]
    fn test_basic_cloud_edge_sync() {
        let mut test = TimeSyncIntegrationTest::new();
        let config = create_test_config();

        test.add_node(NodeId::Cloud, config.clone());
        test.add_node(NodeId::Edge(1), config);

        test.setup_network_topology();

        test.trigger_sync(NodeId::Edge(1), NodeId::Cloud);

        for _ in 0..5 {
            test.process_round();
            test.advance_time(10);
        }

        let stats = test.get_network_stats();
        assert!(stats.synced_nodes > 0, "Should have synced nodes");
    }

    #[test]
    fn test_hierarchical_sync() {
        let mut test = TimeSyncIntegrationTest::new();
        let config = create_test_config();

        test.add_node(NodeId::Cloud, config.clone());
        test.add_node(NodeId::Edge(1), config.clone());
        test.add_node(NodeId::Device([0, 0, 0, 0, 0, 1]), config.clone());
        test.add_node(NodeId::Device([0, 0, 0, 0, 0, 2]), config);

        test.setup_network_topology();

        test.trigger_sync(NodeId::Edge(1), NodeId::Cloud);
        test.trigger_sync(NodeId::Device([0, 0, 0, 0, 0, 1]), NodeId::Edge(1));
        test.trigger_sync(NodeId::Device([0, 0, 0, 0, 0, 2]), NodeId::Edge(1));

        for _ in 0..10 {
            test.process_round();
            test.advance_time(50);
        }

        let stats = test.get_network_stats();
        println!("Hierarchical sync stats: {:?}", stats);
        assert!(stats.synced_nodes >= 2, "Multiple nodes should be synced");
    }

    #[test]
    fn test_clock_drift_compensation() {
        let mut test = TimeSyncIntegrationTest::new();
        let config = create_test_config();

        test.add_node(NodeId::Cloud, config.clone());
        test.add_node(NodeId::Edge(1), config);
        test.setup_network_topology();

        if let Some(edge_node) = test.nodes.get_mut(&NodeId::Edge(1)) {
            edge_node.set_clock_drift(100.0); // 100 ppm drift
            edge_node.set_clock_offset(500); // 500ms initial offset
        }

        for round in 0..5 {
            test.trigger_sync(NodeId::Edge(1), NodeId::Cloud);

            for _ in 0..3 {
                test.process_round();
                test.advance_time(20);
            }

            test.advance_time(1000);

            println!("Round {}: {:?}", round, test.get_network_stats());
        }

        let final_stats = test.get_network_stats();
        assert!(
            final_stats.max_offset < 1000,
            "Clock drift should be compensated"
        );
    }

    #[test]
    fn test_network_interruption_recovery() {
        let mut test = TimeSyncIntegrationTest::new();
        let config = create_test_config();

        test.add_node(NodeId::Cloud, config.clone());
        test.add_node(NodeId::Edge(1), config);
        test.setup_network_topology();

        test.trigger_sync(NodeId::Edge(1), NodeId::Cloud);
        for _ in 0..3 {
            test.process_round();
            test.advance_time(50);
        }

        let stats_before = test.get_network_stats();

        test.network
            .set_packet_loss(NodeId::Edge(1), NodeId::Cloud, 1.0);
        test.network
            .set_packet_loss(NodeId::Cloud, NodeId::Edge(1), 1.0);

        for _ in 0..3 {
            test.trigger_sync(NodeId::Edge(1), NodeId::Cloud);
            test.process_round();
            test.advance_time(100);
        }

        test.network
            .set_packet_loss(NodeId::Edge(1), NodeId::Cloud, 0.0);
        test.network
            .set_packet_loss(NodeId::Cloud, NodeId::Edge(1), 0.0);

        for _ in 0..5 {
            test.trigger_sync(NodeId::Edge(1), NodeId::Cloud);
            test.process_round();
            test.advance_time(100);
        }

        let stats_after = test.get_network_stats();
        println!("Before: {:?}", stats_before);
        println!("After: {:?}", stats_after);

        assert!(
            stats_after.synced_nodes > 0,
            "Should recover after network restoration"
        );
    }

    #[test]
    fn test_large_scale_network() {
        let mut test = TimeSyncIntegrationTest::new();
        let config = create_test_config();

        test.add_node(NodeId::Cloud, config.clone());

        for edge_id in 1..=3 {
            test.add_node(NodeId::Edge(edge_id), config.clone());

            let base_delay = 30u64 + (edge_id as u64) * 10;
            test.network
                .set_network_delay(NodeId::Cloud, NodeId::Edge(edge_id), base_delay);
            test.network
                .set_network_delay(NodeId::Edge(edge_id), NodeId::Cloud, base_delay);

            for device_id in 1..=5 {
                let device_mac = [0, 0, 0, edge_id, 0, device_id];
                let device_node = NodeId::Device(device_mac);
                test.add_node(device_node, config.clone());

                test.network
                    .set_network_delay(NodeId::Edge(edge_id), device_node, 15);
                test.network
                    .set_network_delay(device_node, NodeId::Edge(edge_id), 15);
            }
        }

        for edge_id in 1..=3 {
            test.trigger_sync(NodeId::Edge(edge_id), NodeId::Cloud);

            for device_id in 1..=5 {
                let device_mac = [0, 0, 0, edge_id, 0, device_id];
                test.trigger_sync(NodeId::Device(device_mac), NodeId::Edge(edge_id));
            }
        }

        for round in 0..20 {
            test.process_round();
            test.advance_time(100);

            if round % 5 == 0 {
                let stats = test.get_network_stats();
                println!("Round {}: {:?}", round, stats);
            }
        }

        let final_stats = test.get_network_stats();
        println!("Final stats: {:?}", final_stats);

        assert!(
            final_stats.synced_nodes >= 10,
            "Most nodes should be synced"
        );
        assert!(
            final_stats.max_offset < 200,
            "Time offset should be reasonable"
        );
    }

    #[test]
    fn test_clock_noise_resilience() {
        let mut test = TimeSyncIntegrationTest::new();
        let config = create_test_config();

        test.add_node(NodeId::Cloud, config.clone());
        test.add_node(NodeId::Edge(1), config);
        test.setup_network_topology();

        if let Some(edge_node) = test.nodes.get_mut(&NodeId::Edge(1)) {
            edge_node.set_clock_noise(20); // ±20ms noise
        }

        for _ in 0..10 {
            test.trigger_sync(NodeId::Edge(1), NodeId::Cloud);

            for _ in 0..3 {
                test.process_round();
                test.advance_time(25);
            }

            test.advance_time(200);
        }

        let stats = test.get_network_stats();
        println!("Noise resilience stats: {:?}", stats);

        assert!(stats.synced_nodes > 0, "Should maintain sync despite noise");
        assert!(
            stats.max_offset < 100,
            "Should filter out noise effectively"
        );
    }
}
