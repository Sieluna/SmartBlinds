use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use lumisync_api::message::*;
use lumisync_api::time::{SyncConfig, SyncStatus, TimeProvider, TimeSyncService};
use lumisync_api::transport::{AsyncMessageTransport, Protocol};
use lumisync_api::uuid::DeviceBasedUuidGenerator;
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

type DeviceConnections = Arc<Mutex<HashMap<String, Instant>>>;
type SharedMetrics = Arc<Mutex<EdgeMetrics>>;
type RunningFlag = Arc<AtomicBool>;
type CloudTransport = Arc<Mutex<Option<AsyncMessageTransport<TcpAdapter>>>>;

/// Edge time provider without authoritative time source
pub struct EdgeTimeProvider {
    start_time: Instant,
}

impl EdgeTimeProvider {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }
}

impl TimeProvider for EdgeTimeProvider {
    fn monotonic_time_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    fn wall_clock_time(&self) -> Option<OffsetDateTime> {
        None // Edge node has no authoritative time source
    }

    fn has_authoritative_time(&self) -> bool {
        false
    }
}

/// Edge node configuration
#[derive(Debug, Clone)]
pub struct EdgeNodeConfig {
    pub edge_id: u8,
    pub cloud_server_addr: String,
    pub device_listen_port: u16,
    pub sync_config: SyncConfig,
    pub cloud_sync_interval_ms: u64,
}

impl Default for EdgeNodeConfig {
    fn default() -> Self {
        Self {
            edge_id: 1,
            cloud_server_addr: "127.0.0.1:8080".to_string(),
            device_listen_port: 9090,
            sync_config: SyncConfig {
                sync_interval_ms: 10000, // 10 seconds sync with cloud
                max_drift_ms: 100,       // 100ms drift threshold
                offset_history_size: 5,
                delay_threshold_ms: 50,
                max_retry_count: 3,
                failure_cooldown_ms: 30000,
            },
            cloud_sync_interval_ms: 30000, // 30 seconds
        }
    }
}

/// Edge metrics
#[derive(Debug, Clone, Default)]
pub struct EdgeMetrics {
    pub cloud_sync_requests: u64,
    pub cloud_sync_successes: u64,
    pub cloud_sync_failures: u64,
    pub device_messages_received: u64,
    pub device_messages_sent: u64,
    pub connected_devices: usize,
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

/// Edge node for time synchronization
pub struct EdgeNode {
    config: EdgeNodeConfig,
    time_service: TimeSyncService<EdgeTimeProvider, DeviceBasedUuidGenerator>,
    device_connections: DeviceConnections,
    cloud_transport: CloudTransport,
    metrics: SharedMetrics,
    is_running: RunningFlag,
    sequence_counter: Arc<std::sync::atomic::AtomicU32>,
}

impl EdgeNode {
    pub fn new(config: EdgeNodeConfig) -> Self {
        let time_provider = EdgeTimeProvider::new();
        let node_id = NodeId::Edge(config.edge_id);
        let device_mac = [0xED, 0x6E, 0x00, 0x00, 0x00, config.edge_id];
        let uuid_generator = DeviceBasedUuidGenerator::new(device_mac);

        let time_service = TimeSyncService::new(
            time_provider,
            node_id,
            config.sync_config.clone(),
            uuid_generator,
        );

        Self {
            config,
            time_service,
            device_connections: Arc::new(Mutex::new(HashMap::new())),
            cloud_transport: Arc::new(Mutex::new(None)),
            metrics: Arc::new(Mutex::new(EdgeMetrics::default())),
            is_running: Arc::new(AtomicBool::new(false)),
            sequence_counter: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }

    /// Start edge node service
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.is_running.store(true, Ordering::SeqCst);

        println!("Starting Edge Node {}", self.config.edge_id);

        // Establish persistent connection to cloud
        self.connect_to_cloud().await?;

        // Start device listener service
        self.start_device_listener().await?;

        // Start background tasks
        self.start_cloud_sync_task().await;
        self.start_cloud_connection_manager().await;
        self.start_metrics_task().await;

        // Main event loop for cloud communication
        self.run_cloud_communication().await
    }

    /// Establish persistent connection to cloud
    async fn connect_to_cloud(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!(
            "Connecting to cloud server: {}",
            self.config.cloud_server_addr
        );

        match TcpStream::connect(&self.config.cloud_server_addr).await {
            Ok(stream) => {
                let tcp_adapter = TcpAdapter::new(stream);
                let transport = AsyncMessageTransport::new(tcp_adapter)
                    .with_default_protocol(Protocol::Postcard)
                    .with_crc(true);

                let mut cloud_transport = self.cloud_transport.lock().await;
                *cloud_transport = Some(transport);

                println!("Successfully connected to cloud server");
                Ok(())
            }
            Err(e) => {
                eprintln!("Failed to connect to cloud: {}", e);
                Err(Box::new(e))
            }
        }
    }

    /// Start cloud connection manager to handle reconnection
    async fn start_cloud_connection_manager(&self) {
        let cloud_transport = Arc::clone(&self.cloud_transport);
        let cloud_addr = self.config.cloud_server_addr.clone();
        let is_running = Arc::clone(&self.is_running);
        let metrics = Arc::clone(&self.metrics);

        tokio::spawn(async move {
            let mut reconnect_interval =
                tokio::time::interval(tokio::time::Duration::from_secs(30));

            while is_running.load(Ordering::SeqCst) {
                reconnect_interval.tick().await;

                // Check if connection is still alive
                let needs_reconnect = {
                    let transport_guard = cloud_transport.lock().await;
                    transport_guard.is_none()
                };

                if needs_reconnect {
                    println!("Attempting to reconnect to cloud...");

                    match TcpStream::connect(&cloud_addr).await {
                        Ok(stream) => {
                            let tcp_adapter = TcpAdapter::new(stream);
                            let transport = AsyncMessageTransport::new(tcp_adapter)
                                .with_default_protocol(Protocol::Postcard)
                                .with_crc(true);

                            let mut cloud_transport_guard = cloud_transport.lock().await;
                            *cloud_transport_guard = Some(transport);

                            println!("Successfully reconnected to cloud");
                        }
                        Err(e) => {
                            eprintln!("Failed to reconnect to cloud: {}", e);
                            let mut metrics = metrics.lock().await;
                            metrics.cloud_sync_failures += 1;
                        }
                    }
                }
            }
        });
    }

    /// Start device listener service
    async fn start_device_listener(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener =
            TcpListener::bind(format!("0.0.0.0:{}", self.config.device_listen_port)).await?;
        let device_connections = Arc::clone(&self.device_connections);
        let metrics = Arc::clone(&self.metrics);
        let is_running = Arc::clone(&self.is_running);
        let listen_port = self.config.device_listen_port;

        tokio::spawn(async move {
            println!("Device listener started on port {}", listen_port);

            while is_running.load(Ordering::SeqCst) {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        println!("Device connected from: {}", addr);
                        let connections = Arc::clone(&device_connections);
                        let metrics = Arc::clone(&metrics);

                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_device_connection(
                                stream,
                                addr.to_string(),
                                connections,
                                metrics,
                            )
                            .await
                            {
                                eprintln!("Error handling device connection from {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to accept device connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle device connection
    async fn handle_device_connection(
        stream: TcpStream,
        addr: String,
        device_connections: DeviceConnections,
        metrics: SharedMetrics,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tcp_adapter = TcpAdapter::new(stream);
        let mut transport = AsyncMessageTransport::new(tcp_adapter)
            .with_default_protocol(Protocol::Postcard)
            .with_crc(true);

        // Register device connection
        {
            let mut connections = device_connections.lock().await;
            connections.insert(addr.clone(), Instant::now());
        }

        loop {
            match transport.receive_message::<Message>().await {
                Ok((message, _protocol, _stream_id)) => {
                    println!(
                        "Received message from device {}: {:?}",
                        addr, message.header.source
                    );

                    // Update device last activity time
                    {
                        let mut connections = device_connections.lock().await;
                        connections.insert(addr.clone(), Instant::now());
                    }

                    // Process device message
                    if let Some(response) = Self::process_device_message(&message).await {
                        if let Err(e) = transport
                            .send_message(&response, Some(Protocol::Postcard), None)
                            .await
                        {
                            eprintln!("Failed to send response to device: {}", e);
                            break;
                        }
                        println!("Sent response to device {}", addr);

                        // Update metrics
                        {
                            let mut metrics = metrics.lock().await;
                            metrics.device_messages_sent += 1;
                        }
                    }

                    // Update metrics
                    {
                        let mut metrics = metrics.lock().await;
                        metrics.device_messages_received += 1;
                    }
                }
                Err(e) => {
                    eprintln!("Failed to receive message from device {}: {}", addr, e);
                    break;
                }
            }
        }

        // Cleanup device connection
        {
            let mut connections = device_connections.lock().await;
            connections.remove(&addr);
        }

        println!("Device disconnected: {}", addr);
        Ok(())
    }

    /// Process device message
    async fn process_device_message(message: &Message) -> Option<Message> {
        match &message.payload {
            MessagePayload::TimeSync(TimeSyncPayload::Request { sequence, .. }) => {
                // Simple time sync response for demonstration
                Some(Message {
                    header: MessageHeader {
                        id: uuid::Uuid::new_v4(),
                        timestamp: OffsetDateTime::now_utc(),
                        priority: Priority::Regular,
                        source: NodeId::Edge(1),
                        target: message.header.source,
                    },
                    payload: MessagePayload::TimeSync(TimeSyncPayload::Response {
                        request_sequence: *sequence,
                        request_receive_time: OffsetDateTime::now_utc(),
                        response_send_time: OffsetDateTime::now_utc(),
                        estimated_delay_ms: 20,
                        accuracy_ms: 10,
                    }),
                })
            }
            MessagePayload::TimeSync(TimeSyncPayload::StatusQuery) => Some(Message {
                header: MessageHeader {
                    id: uuid::Uuid::new_v4(),
                    timestamp: OffsetDateTime::now_utc(),
                    priority: Priority::Regular,
                    source: NodeId::Edge(1),
                    target: message.header.source,
                },
                payload: MessagePayload::TimeSync(TimeSyncPayload::StatusResponse {
                    is_synced: true,
                    current_offset_ms: 0,
                    last_sync_time: OffsetDateTime::now_utc(),
                    accuracy_ms: 10,
                }),
            }),
            _ => {
                println!(
                    "Received other message type from device: {:?}",
                    message.payload
                );
                None
            }
        }
    }

    /// Start cloud sync task with persistent connection
    async fn start_cloud_sync_task(&self) {
        let sync_interval = self.config.cloud_sync_interval_ms;
        let cloud_transport = Arc::clone(&self.cloud_transport);
        let metrics = Arc::clone(&self.metrics);
        let is_running = Arc::clone(&self.is_running);
        let sequence_counter = Arc::clone(&self.sequence_counter);

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_millis(sync_interval));

            while is_running.load(Ordering::SeqCst) {
                interval.tick().await;

                match Self::perform_cloud_sync_persistent(&cloud_transport, &sequence_counter).await
                {
                    Ok(_) => {
                        let mut metrics = metrics.lock().await;
                        metrics.cloud_sync_successes += 1;
                        println!("Cloud sync successful (persistent connection)");
                    }
                    Err(e) => {
                        eprintln!("Cloud sync failed: {}", e);
                        let mut metrics = metrics.lock().await;
                        metrics.cloud_sync_failures += 1;

                        // Mark connection as invalid on failure
                        let mut transport_guard = cloud_transport.lock().await;
                        *transport_guard = None;
                    }
                }

                {
                    let mut metrics = metrics.lock().await;
                    metrics.cloud_sync_requests += 1;
                }
            }
        });
    }

    /// Perform cloud sync using persistent connection
    async fn perform_cloud_sync_persistent(
        cloud_transport: &CloudTransport,
        sequence_counter: &Arc<std::sync::atomic::AtomicU32>,
    ) -> Result<(), String> {
        let mut transport_guard = cloud_transport.lock().await;

        if let Some(ref mut transport) = transport_guard.as_mut() {
            let sequence = sequence_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            // Create time sync request
            let request = Message {
                header: MessageHeader {
                    id: uuid::Uuid::new_v4(),
                    timestamp: OffsetDateTime::UNIX_EPOCH,
                    priority: Priority::Regular,
                    source: NodeId::Edge(1),
                    target: NodeId::Cloud,
                },
                payload: MessagePayload::TimeSync(TimeSyncPayload::Request {
                    sequence,
                    send_time: None,
                    precision_ms: 10,
                }),
            };

            // Send request
            transport
                .send_message(&request, Some(Protocol::Postcard), None)
                .await
                .map_err(|e| format!("Failed to send sync request: {}", e))?;

            // Receive response
            let (_response, _, _): (Message, _, _) = transport
                .receive_message()
                .await
                .map_err(|e| format!("Failed to receive sync response: {}", e))?;

            Ok(())
        } else {
            Err("No active cloud connection".to_string())
        }
    }

    /// Start metrics collection task
    async fn start_metrics_task(&self) {
        let metrics = Arc::clone(&self.metrics);
        let device_connections = Arc::clone(&self.device_connections);
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

            while is_running.load(Ordering::SeqCst) {
                interval.tick().await;

                // Update connected device count
                {
                    let connections = device_connections.lock().await;
                    let mut metrics = metrics.lock().await;
                    metrics.connected_devices = connections.len();
                }

                let metrics_snapshot = {
                    let metrics = metrics.lock().await;
                    metrics.clone()
                };

                println!(
                    "Edge Metrics - Connected devices: {}, Cloud syncs: {}/{}, Device messages: {}",
                    metrics_snapshot.connected_devices,
                    metrics_snapshot.cloud_sync_successes,
                    metrics_snapshot.cloud_sync_requests,
                    metrics_snapshot.device_messages_received
                );
            }
        });
    }

    /// Main event loop for cloud communication
    async fn run_cloud_communication(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        while self.is_running.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Optionally handle incoming messages from cloud here
            // For now, just maintain the event loop
        }
        Ok(())
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> EdgeMetrics {
        self.metrics.lock().await.clone()
    }

    /// Get sync status
    pub fn get_sync_status(&self) -> SyncStatus {
        self.time_service.get_sync_status()
    }

    /// Graceful shutdown
    pub async fn shutdown(&mut self) {
        println!("Shutting down edge node...");
        self.is_running.store(false, Ordering::SeqCst);

        let connections = self.device_connections.lock().await;
        println!("Closing {} device connections", connections.len());
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut config = EdgeNodeConfig::default();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--edge-id" | "-e" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<u8>() {
                        Ok(id) => {
                            config.edge_id = id;
                            println!("Using edge ID: {}", id);
                        }
                        Err(_) => {
                            eprintln!("Error: Invalid edge ID '{}'", args[i + 1]);
                            std::process::exit(1);
                        }
                    }
                    i += 1;
                } else {
                    eprintln!("Error: --edge-id requires a value");
                    std::process::exit(1);
                }
            }
            "--cloud-addr" | "-c" => {
                if i + 1 < args.len() {
                    config.cloud_server_addr = args[i + 1].clone();
                    println!("Using cloud address: {}", config.cloud_server_addr);
                    i += 1;
                } else {
                    eprintln!("Error: --cloud-addr requires a value");
                    std::process::exit(1);
                }
            }
            "--device-port" | "-d" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<u16>() {
                        Ok(port) => {
                            config.device_listen_port = port;
                            println!("Using device port: {}", port);
                        }
                        Err(_) => {
                            eprintln!("Error: Invalid device port '{}'", args[i + 1]);
                            std::process::exit(1);
                        }
                    }
                    i += 1;
                } else {
                    eprintln!("Error: --device-port requires a value");
                    std::process::exit(1);
                }
            }
            "--sync-interval" | "-s" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<u64>() {
                        Ok(interval) => {
                            config.cloud_sync_interval_ms = interval * 1000; // Convert seconds to ms
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
            "--help" | "-h" => {
                println!("Time Edge Node");
                println!("Usage: {} [OPTIONS]", args[0]);
                println!();
                println!("Options:");
                println!("  --edge-id, -e <ID>           Set edge node ID (default: 1)");
                println!("  --cloud-addr, -c <ADDR>      Set cloud server address (default: 127.0.0.1:8080)");
                println!("  --device-port, -d <PORT>     Set device listen port (default: 9090)");
                println!("  --sync-interval, -s <SEC>    Set cloud sync interval in seconds (default: 30)");
                println!("  --help, -h                   Show this help message");
                println!();
                println!("Examples:");
                println!("  {} --edge-id 2 --device-port 9091", args[0]);
                println!("  {} --cloud-addr 192.168.1.100:8080", args[0]);
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

    let mut edge_node = EdgeNode::new(config.clone());

    println!("Starting edge node...");
    println!("This node synchronizes time with cloud and provides time sync to devices");
    println!("Edge ID: {}", config.edge_id);
    println!("Cloud address: {}", config.cloud_server_addr);
    println!("Device port: {}", config.device_listen_port);

    edge_node.start().await
}
