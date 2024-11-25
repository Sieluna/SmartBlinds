use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use lumisync_api::message::*;
use lumisync_api::time::TimeProvider;
use lumisync_api::transport::{AsyncMessageTransport, Protocol};
use lumisync_api::uuid::{DeviceBasedUuidGenerator, UuidGenerator};
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::signal;
use tokio::sync::{Mutex, mpsc};

type SharedTimeSync = Arc<Mutex<DeviceTimeSync>>;
type SharedMetrics = Arc<Mutex<DeviceMetrics>>;
type RunningFlag = Arc<AtomicBool>;
type SequenceCounter = Arc<AtomicU32>;
type MessageSender = mpsc::UnboundedSender<Message>;

/// Device time provider without authoritative time source
pub struct DeviceTimeProvider {
    start_time: Instant,
}

impl DeviceTimeProvider {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    pub fn uptime_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

impl TimeProvider for DeviceTimeProvider {
    fn monotonic_time_ms(&self) -> u64 {
        self.uptime_ms()
    }

    fn wall_clock_time(&self) -> Option<OffsetDateTime> {
        None // Device has no authoritative time source
    }

    fn has_authoritative_time(&self) -> bool {
        false
    }
}

/// Device time synchronization handler
pub struct DeviceTimeSync {
    time_provider: DeviceTimeProvider,
    time_offset_ms: i64,
    last_sync_time: Option<u64>,
    sync_state: DeviceSyncState,
    sync_expiry_threshold_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceSyncState {
    Unsynced,
    Synced,
    Expired,
}

impl DeviceTimeSync {
    pub fn new() -> Self {
        Self {
            time_provider: DeviceTimeProvider::new(),
            time_offset_ms: 0,
            last_sync_time: None,
            sync_state: DeviceSyncState::Unsynced,
            sync_expiry_threshold_ms: 300_000, // 5 minutes expiry
        }
    }

    /// Handle time sync response
    pub fn handle_time_sync_response(&mut self, message: &Message) -> Result<(), &'static str> {
        if let MessagePayload::TimeSync(TimeSyncPayload::Response {
            response_send_time, ..
        }) = &message.payload
        {
            let current_uptime = self.time_provider.uptime_ms();

            // Calculate time offset
            let response_time_ms = response_send_time.unix_timestamp() as u64 * 1000
                + response_send_time.millisecond() as u64;
            let new_offset = response_time_ms as i64 - current_uptime as i64;

            self.time_offset_ms = new_offset;
            self.last_sync_time = Some(current_uptime);
            self.sync_state = DeviceSyncState::Synced;

            Ok(())
        } else {
            Err("Invalid time sync response message")
        }
    }

    /// Get current synchronized time
    pub fn get_current_time(&self) -> OffsetDateTime {
        let current_uptime = self.time_provider.uptime_ms();
        let adjusted_time = (current_uptime as i64 + self.time_offset_ms) as u64;

        OffsetDateTime::from_unix_timestamp_nanos((adjusted_time as i128) * 1_000_000)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }

    /// Get relative timestamp
    pub fn get_relative_timestamp(&self) -> u64 {
        self.time_provider.uptime_ms()
    }

    /// Check if synchronized
    pub fn is_synced(&self) -> bool {
        if let Some(last_sync) = self.last_sync_time {
            let current_time = self.time_provider.uptime_ms();
            current_time.saturating_sub(last_sync) <= self.sync_expiry_threshold_ms
                && self.sync_state == DeviceSyncState::Synced
        } else {
            false
        }
    }

    /// Update sync state
    pub fn update_sync_state(&mut self) {
        if let Some(last_sync) = self.last_sync_time {
            let current_time = self.time_provider.uptime_ms();
            if current_time.saturating_sub(last_sync) > self.sync_expiry_threshold_ms {
                self.sync_state = DeviceSyncState::Expired;
            }
        }
    }
}

/// Device configuration
#[derive(Debug, Clone)]
pub struct DeviceConfig {
    pub device_mac: [u8; 6],
    pub edge_server_addr: String,
    pub sync_request_interval_ms: u64,
    pub status_report_interval_ms: u64,
    pub max_reconnect_attempts: u8,
    pub connection_timeout_ms: u64,
    pub target_edge_id: u8,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            device_mac: [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC],
            edge_server_addr: "127.0.0.1:9090".to_string(),
            sync_request_interval_ms: 60000,  // 1 minute sync request
            status_report_interval_ms: 30000, // 30 seconds status report
            max_reconnect_attempts: 5,
            connection_timeout_ms: 5000,
            target_edge_id: 1, // Default to edge 1
        }
    }
}

/// Device metrics
#[derive(Debug, Default, Clone)]
pub struct DeviceMetrics {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub connection_failures: u64,
    pub sync_updates: u64,
    pub last_edge_contact: Option<Instant>,
    pub uptime_ms: u64,
}

/// TCP adapter for async transport
pub struct TcpAdapter {
    stream: TcpStream,
}

impl TcpAdapter {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }
}

impl embedded_io_async::ErrorType for TcpAdapter {
    type Error = std::io::Error;
}

impl embedded_io_async::Read for TcpAdapter {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.stream.read(buf).await
    }
}

impl embedded_io_async::Write for TcpAdapter {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.stream.write(buf).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.stream.flush().await
    }
}

/// Simple device for time synchronization demonstration
pub struct SimpleDevice {
    config: DeviceConfig,
    time_sync: SharedTimeSync,
    edge_transport: Option<AsyncMessageTransport<TcpAdapter>>,
    metrics: SharedMetrics,
    is_running: RunningFlag,
    uuid_generator: DeviceBasedUuidGenerator,
    sequence_counter: SequenceCounter,
    message_sender: Option<MessageSender>,
}

impl SimpleDevice {
    pub fn new(config: DeviceConfig) -> Self {
        let time_sync = DeviceTimeSync::new();
        let uuid_generator = DeviceBasedUuidGenerator::new(config.device_mac);

        Self {
            config,
            time_sync: Arc::new(Mutex::new(time_sync)),
            edge_transport: None,
            metrics: Arc::new(Mutex::new(DeviceMetrics::default())),
            is_running: Arc::new(AtomicBool::new(false)),
            uuid_generator,
            sequence_counter: Arc::new(AtomicU32::new(0)),
            message_sender: None,
        }
    }

    /// Start device
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.is_running.store(true, Ordering::SeqCst);

        println!(
            "[DEVICE-{:02X}] Starting device -> Edge({})",
            self.config.device_mac[5], self.config.target_edge_id
        );

        let (message_tx, message_rx) = mpsc::unbounded_channel();
        self.message_sender = Some(message_tx);

        self.connect_to_edge().await?;
        self.start_sync_request_task().await;
        self.start_status_report_task().await;
        self.start_metrics_collection_task().await;

        // Run main loop with integrated shutdown handling
        self.run_main_loop(message_rx).await?;

        self.shutdown().await;
        Ok(())
    }

    /// Connect to edge node
    async fn connect_to_edge(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let stream = TcpStream::connect(&self.config.edge_server_addr).await?;
        let tcp_adapter = TcpAdapter::new(stream);
        let transport = AsyncMessageTransport::new(tcp_adapter)
            .with_default_protocol(Protocol::Postcard)
            .with_crc(true);

        self.edge_transport = Some(transport);

        // Update metrics
        {
            let mut metrics = self.metrics.lock().await;
            metrics.last_edge_contact = Some(Instant::now());
        }

        println!(
            "[DEVICE-{:02X}] Connected to {}",
            self.config.device_mac[5], self.config.edge_server_addr
        );
        Ok(())
    }

    /// Start time sync request task
    async fn start_sync_request_task(&self) {
        let sync_interval = self.config.sync_request_interval_ms;
        let time_sync = Arc::clone(&self.time_sync);
        let metrics = Arc::clone(&self.metrics);
        let is_running = Arc::clone(&self.is_running);
        let device_mac = self.config.device_mac;
        let target_edge_id = self.config.target_edge_id;
        let uuid_generator = self.uuid_generator.clone();
        let sequence_counter = Arc::clone(&self.sequence_counter);
        let message_sender = self.message_sender.clone().unwrap();

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_millis(sync_interval));

            while is_running.load(Ordering::SeqCst) {
                interval.tick().await;

                // Create sync request message with updated sequence counter
                let sequence = sequence_counter.fetch_add(1, Ordering::SeqCst);
                let message = {
                    let time_sync = time_sync.lock().await;

                    Message {
                        header: MessageHeader {
                            id: uuid_generator.generate(),
                            timestamp: time_sync.get_current_time(),
                            priority: Priority::Regular,
                            source: NodeId::Device(device_mac),
                            target: NodeId::Edge(target_edge_id),
                        },
                        payload: MessagePayload::TimeSync(TimeSyncPayload::Request {
                            sequence,
                            send_time: if time_sync.is_synced() {
                                Some(time_sync.get_current_time())
                            } else {
                                None
                            },
                            precision_ms: 50,
                        }),
                    }
                };

                // Send message through channel to main loop
                if message_sender.send(message).is_err() {
                    break;
                }

                {
                    let mut metrics = metrics.lock().await;
                    metrics.messages_sent += 1;
                }
            }
        });
    }

    /// Start status report task
    async fn start_status_report_task(&self) {
        let report_interval = self.config.status_report_interval_ms;
        let time_sync = Arc::clone(&self.time_sync);
        let is_running = Arc::clone(&self.is_running);
        let device_mac = self.config.device_mac;
        let target_edge_id = self.config.target_edge_id;
        let uuid_generator = self.uuid_generator.clone();
        let message_sender = self.message_sender.clone().unwrap();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(report_interval));

            while is_running.load(Ordering::SeqCst) {
                interval.tick().await;

                let message = {
                    let time_sync = time_sync.lock().await;

                    Message {
                        header: MessageHeader {
                            id: uuid_generator.generate(),
                            timestamp: time_sync.get_current_time(),
                            priority: Priority::Regular,
                            source: NodeId::Device(device_mac),
                            target: NodeId::Edge(target_edge_id),
                        },
                        payload: MessagePayload::TimeSync(TimeSyncPayload::StatusQuery),
                    }
                };

                if message_sender.send(message).is_err() {
                    break;
                }
            }
        });
    }

    /// Start metrics collection task
    async fn start_metrics_collection_task(&self) {
        let metrics = Arc::clone(&self.metrics);
        let time_sync = Arc::clone(&self.time_sync);
        let is_running = Arc::clone(&self.is_running);
        let device_mac = self.config.device_mac;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(90));

            while is_running.load(Ordering::SeqCst) {
                interval.tick().await;

                // Update uptime
                {
                    let time_sync = time_sync.lock().await;
                    let mut metrics = metrics.lock().await;
                    metrics.uptime_ms = time_sync.get_relative_timestamp();
                }

                let (metrics_snapshot, is_synced) = {
                    let metrics = metrics.lock().await;
                    let time_sync = time_sync.lock().await;
                    (metrics.clone(), time_sync.is_synced())
                };

                println!(
                    "[DEVICE-{:02X}] Status: {}, {}msg sent/{}recv, {}s uptime",
                    device_mac[5],
                    if is_synced { "SYNCED" } else { "UNSYNCED" },
                    metrics_snapshot.messages_sent,
                    metrics_snapshot.messages_received,
                    metrics_snapshot.uptime_ms / 1000
                );
            }
        });
    }

    /// Main event loop
    async fn run_main_loop(
        &mut self,
        mut message_rx: mpsc::UnboundedReceiver<Message>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        while self.is_running.load(Ordering::SeqCst) {
            tokio::select! {
                // Handle shutdown signals
                _ = signal::ctrl_c() => {
                    println!("[DEVICE-{:02X}] Shutdown signal received", self.config.device_mac[5]);
                    self.is_running.store(false, Ordering::SeqCst);
                    break;
                }
                // Handle outgoing messages from background tasks
                Some(message) = message_rx.recv() => {
                    if let Some(ref mut transport) = self.edge_transport {
                        if let Err(e) = transport.send_message(&message, Some(Protocol::Postcard), None).await {
                            eprintln!("[ERROR] Device-{:02X}: Send failed: {}", self.config.device_mac[5], e);

                            if let Err(e) = self.connect_to_edge().await {
                                eprintln!("[ERROR] Device-{:02X}: Reconnect failed: {}", self.config.device_mac[5], e);
                                let mut metrics = self.metrics.lock().await;
                                metrics.connection_failures += 1;
                                tokio::time::sleep(Duration::from_secs(5)).await;
                            }
                        }
                    }
                }
                // Handle incoming messages from edge node
                _ = self.handle_incoming_messages() => {
                    // Handle incoming messages result if needed
                }
                // Periodic maintenance
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    let mut time_sync = self.time_sync.lock().await;
                    time_sync.update_sync_state();
                }
            }
        }

        Ok(())
    }

    /// Handle incoming messages from edge
    async fn handle_incoming_messages(&mut self) {
        if let Some(ref mut transport) = self.edge_transport {
            match transport.receive_message::<Message>().await {
                Ok((message, _protocol, _stream_id)) => {
                    if let Err(e) = self.process_edge_message(&message).await {
                        eprintln!(
                            "[ERROR] Device-{:02X}: Message processing error: {}",
                            self.config.device_mac[5], e
                        );
                    }

                    // Update metrics
                    {
                        let mut metrics = self.metrics.lock().await;
                        metrics.messages_received += 1;
                        metrics.last_edge_contact = Some(Instant::now());
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[ERROR] Device-{:02X}: Receive error: {}",
                        self.config.device_mac[5], e
                    );

                    // Try to reconnect
                    if let Err(e) = self.connect_to_edge().await {
                        eprintln!(
                            "[ERROR] Device-{:02X}: Reconnect failed: {}",
                            self.config.device_mac[5], e
                        );

                        // Update failure metrics
                        {
                            let mut metrics = self.metrics.lock().await;
                            metrics.connection_failures += 1;
                        }

                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        }
    }

    /// Process message from edge node
    async fn process_edge_message(
        &mut self,
        message: &Message,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match &message.payload {
            MessagePayload::TimeSync(sync_payload) => match sync_payload {
                TimeSyncPayload::Response { .. } => {
                    let mut time_sync = self.time_sync.lock().await;
                    if let Err(e) = time_sync.handle_time_sync_response(message) {
                        eprintln!(
                            "[ERROR] Device-{:02X}: Sync response error: {}",
                            self.config.device_mac[5], e
                        );
                    } else {
                        let mut metrics = self.metrics.lock().await;
                        metrics.sync_updates += 1;
                        println!(
                            "[SUCCESS] Device-{:02X}: Time synchronized",
                            self.config.device_mac[5]
                        );
                    }
                }
                TimeSyncPayload::StatusResponse { is_synced, .. } => {
                    if !is_synced {
                        println!(
                            "[WARN] Device-{:02X}: Edge not synchronized",
                            self.config.device_mac[5]
                        );
                    }
                }
                _ => {
                    // Other time sync message types - no logging needed
                }
            },
            _ => {
                // Other message types - no logging needed
            }
        }
        Ok(())
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> DeviceMetrics {
        self.metrics.lock().await.clone()
    }

    /// Get sync status
    pub async fn get_sync_status(&self) -> bool {
        self.time_sync.lock().await.is_synced()
    }

    /// Graceful shutdown
    pub async fn shutdown(&mut self) {
        println!("[DEVICE-{:02X}] Shutting down", self.config.device_mac[5]);
        self.is_running.store(false, Ordering::SeqCst);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = DeviceConfig::default();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--edge-addr" | "-e" => {
                if i + 1 < args.len() {
                    config.edge_server_addr = args[i + 1].clone();
                    println!("Using edge address: {}", config.edge_server_addr);
                    i += 1;
                } else {
                    eprintln!("Error: --edge-addr requires a value");
                    std::process::exit(1);
                }
            }
            "--device-mac" | "-m" => {
                if i + 1 < args.len() {
                    let mac_str = &args[i + 1];
                    if let Ok(mac) = parse_mac_address(mac_str) {
                        config.device_mac = mac;
                        println!(
                            "Using device MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
                        );
                    } else {
                        eprintln!("Error: Invalid MAC address format '{}'", mac_str);
                        eprintln!("Expected format: AA:BB:CC:DD:EE:FF or AABBCCDDEEFF");
                        std::process::exit(1);
                    }
                    i += 1;
                } else {
                    eprintln!("Error: --device-mac requires a value");
                    std::process::exit(1);
                }
            }
            "--sync-interval" | "-s" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<u64>() {
                        Ok(interval) => {
                            config.sync_request_interval_ms = interval * 1000; // Convert seconds to ms
                            println!("Using sync interval: {}s", interval);
                        }
                        Err(_) => {
                            eprintln!("Error: Invalid sync interval '{}'", args[i + 1]);
                            std::process::exit(1);
                        }
                    }
                    i += 1;
                } else {
                    eprintln!("Error: --sync-interval requires a value");
                    std::process::exit(1);
                }
            }
            "--status-interval" | "-r" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<u64>() {
                        Ok(interval) => {
                            config.status_report_interval_ms = interval * 1000; // Convert seconds to ms
                            println!("Using status report interval: {}s", interval);
                        }
                        Err(_) => {
                            eprintln!("Error: Invalid status interval '{}'", args[i + 1]);
                            std::process::exit(1);
                        }
                    }
                    i += 1;
                } else {
                    eprintln!("Error: --status-interval requires a value");
                    std::process::exit(1);
                }
            }
            "--target-edge" | "-t" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<u8>() {
                        Ok(edge_id) => {
                            config.target_edge_id = edge_id;
                            println!("Using target edge ID: {}", edge_id);
                        }
                        Err(_) => {
                            eprintln!("Error: Invalid target edge ID '{}'", args[i + 1]);
                            std::process::exit(1);
                        }
                    }
                    i += 1;
                } else {
                    eprintln!("Error: --target-edge requires a value");
                    std::process::exit(1);
                }
            }
            "--help" | "-h" => {
                println!("Time Device Node");
                println!("Usage: {} [OPTIONS]", args[0]);
                println!();
                println!("Options:");
                println!("  --edge-addr, -e <ADDR>       Set edge server address (default: 127.0.0.1:9090)");
                println!("  --device-mac, -m <MAC>       Set device MAC address (default: 12:34:56:78:9A:BC)");
                println!("  --sync-interval, -s <SEC>    Set sync request interval in seconds (default: 60)");
                println!("  --status-interval, -r <SEC>  Set status report interval in seconds (default: 30)");
                println!("  --target-edge, -t <ID>       Set target edge node ID (default: 1)");
                println!("  --help, -h                   Show this help message");
                println!();
                println!("Examples:");
                println!("  {} --device-mac AA:BB:CC:DD:EE:FF", args[0]);
                println!("  {} --edge-addr 192.168.1.101:9090 --sync-interval 15", args[0]);
                std::process::exit(0);
            }
            arg if arg.starts_with("-") => {
                eprintln!("Error: Unknown option '{}'", arg);
                eprintln!("Use --help for usage information");
                std::process::exit(1);
            }
            _ => {}
        }
        i += 1;
    }

    let mut simple_device = SimpleDevice::new(config.clone());

    println!("Starting device...");
    println!(
        "This device synchronizes time with edge node and demonstrates transport functionality"
    );
    println!(
        "Device MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        config.device_mac[0],
        config.device_mac[1],
        config.device_mac[2],
        config.device_mac[3],
        config.device_mac[4],
        config.device_mac[5]
    );
    println!("Edge address: {}", config.edge_server_addr);

    simple_device.start().await
}

/// Parse MAC address from string format
fn parse_mac_address(mac_str: &str) -> Result<[u8; 6], ()> {
    // Remove colons and convert to uppercase
    let clean_mac = mac_str.replace(":", "").replace("-", "").to_uppercase();

    if clean_mac.len() != 12 {
        return Err(());
    }

    let mut mac = [0u8; 6];
    for i in 0..6 {
        let hex_pair = &clean_mac[i * 2..i * 2 + 2];
        mac[i] = u8::from_str_radix(hex_pair, 16).map_err(|_| ())?;
    }

    Ok(mac)
}
