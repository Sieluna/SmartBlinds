use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use lumisync_api::message::*;
use lumisync_api::time::{SyncConfig, TimeProvider, TimeSyncCoordinator, TimeSyncService};
use lumisync_api::transport::{AsyncMessageTransport, Protocol};
use lumisync_api::uuid::DeviceBasedUuidGenerator;
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use tokio::sync::Mutex;

type DeviceConnections = Arc<Mutex<HashMap<String, Instant>>>;
type SharedCoordinator =
    Arc<Mutex<TimeSyncCoordinator<EdgeTimeProvider, DeviceBasedUuidGenerator>>>;

#[derive(Clone)]
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
        None
    }

    fn has_authoritative_time(&self) -> bool {
        false
    }
}

/// Simple edge configuration
#[derive(Debug, Clone)]
pub struct EdgeConfig {
    pub edge_id: u8,
    pub cloud_addr: String,
    pub device_port: u16,
    pub max_devices: usize,
}

impl Default for EdgeConfig {
    fn default() -> Self {
        Self {
            edge_id: 1,
            cloud_addr: "127.0.0.1:8080".to_string(),
            device_port: 9090,
            max_devices: 10,
        }
    }
}

/// TCP adapter for transport
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

/// Simple edge node with coordinator
pub struct SimpleEdgeNode {
    config: EdgeConfig,
    coordinator: SharedCoordinator,
    connections: DeviceConnections,
    is_running: Arc<AtomicBool>,
}

impl SimpleEdgeNode {
    pub fn new(config: EdgeConfig) -> Self {
        // Create coordinator
        let mut coordinator = TimeSyncCoordinator::new();

        // Create edge service
        let time_provider = EdgeTimeProvider::new();
        let edge_node_id = NodeId::Edge(config.edge_id);
        let device_mac = [0xED, 0x6E, 0x00, 0x00, 0x00, config.edge_id];
        let uuid_generator = DeviceBasedUuidGenerator::new(device_mac);

        let sync_config = SyncConfig {
            sync_interval_ms: 30000,
            max_drift_ms: 100,
            offset_history_size: 5,
            delay_threshold_ms: 50,
            max_retry_count: 3,
            failure_cooldown_ms: 30000,
        };

        let edge_service =
            TimeSyncService::new(time_provider, edge_node_id, sync_config, uuid_generator);

        // Add edge service to coordinator
        coordinator.add_service(edge_node_id, edge_service);

        Self {
            config,
            coordinator: Arc::new(Mutex::new(coordinator)),
            connections: Arc::new(Mutex::new(HashMap::new())),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the edge node
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.is_running.store(true, Ordering::SeqCst);

        println!("[EDGE-{}] Starting edge node", self.config.edge_id);
        println!(
            "[EDGE-{}] Device port: {}",
            self.config.edge_id, self.config.device_port
        );
        println!(
            "[EDGE-{}] Cloud: {}",
            self.config.edge_id, self.config.cloud_addr
        );

        // Start device listener
        self.start_device_listener().await?;

        // Start cloud sync task
        self.start_cloud_sync().await;

        // Start status monitoring
        self.start_status_monitor().await;

        // Main loop
        loop {
            tokio::select! {
                _ = signal::ctrl_c() => {
                    println!("[EDGE-{}] Shutdown signal received", self.config.edge_id);
                    break;
                }
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    if !self.is_running.load(Ordering::SeqCst) {
                        break;
                    }
                }
            }
        }

        self.shutdown().await;
        Ok(())
    }

    /// Start device listener
    async fn start_device_listener(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.config.device_port)).await?;
        let connections = Arc::clone(&self.connections);
        let coordinator = Arc::clone(&self.coordinator);
        let is_running = Arc::clone(&self.is_running);
        let config = self.config.clone();

        tokio::spawn(async move {
            println!("[EDGE-{}] Ready for device connections", config.edge_id);

            while is_running.load(Ordering::SeqCst) {
                tokio::select! {
                    result = listener.accept() => {
                        if let Ok((stream, addr)) = result {
                            println!("[EDGE-{}] Device connected: {}", config.edge_id, addr);

                            let connections = Arc::clone(&connections);
                            let coordinator = Arc::clone(&coordinator);
                            let is_running = Arc::clone(&is_running);
                            let max_devices = config.max_devices;
                            let edge_id = config.edge_id;

                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_device(
                                    stream,
                                    addr.to_string(),
                                    connections,
                                    coordinator,
                                    is_running,
                                    max_devices,
                                    edge_id,
                                ).await {
                                    eprintln!("[ERROR] Edge-{}: Device {} error: {}", edge_id, addr, e);
                                }
                            });
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        if !is_running.load(Ordering::SeqCst) {
                            break;
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle device connection
    async fn handle_device(
        stream: TcpStream,
        addr: String,
        connections: DeviceConnections,
        coordinator: SharedCoordinator,
        is_running: Arc<AtomicBool>,
        max_devices: usize,
        edge_id: u8,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tcp_adapter = TcpAdapter::new(stream);
        let mut transport = AsyncMessageTransport::new(tcp_adapter)
            .with_default_protocol(Protocol::Postcard)
            .with_crc(true);

        // Add connection
        {
            let mut connections = connections.lock().await;
            connections.insert(addr.clone(), Instant::now());
        }

        let mut device_node_id: Option<NodeId> = None;
        let mut message_count = 0u64;

        while is_running.load(Ordering::SeqCst) {
            tokio::select! {
                result = transport.receive_message::<Message>() => {
                    match result {
                        Ok((message, _protocol, _stream_id)) => {
                            message_count += 1;

                            // Update connection time
                            {
                                let mut connections = connections.lock().await;
                                connections.insert(addr.clone(), Instant::now());
                            }

                            // Add device service if not exists
                            if device_node_id.is_none() {
                                match Self::add_device_service(
                                    &coordinator,
                                    &message,
                                    max_devices,
                                ).await {
                                    Ok(node_id) => {
                                        device_node_id = Some(node_id);
                                        println!("[EDGE-{}] Device registered: {:?}", edge_id, node_id);
                                    }
                                    Err(e) => {
                                        eprintln!("[ERROR] Edge-{}: Failed to register device: {}", edge_id, e);
                                    }
                                }
                            }

                            // Process message through coordinator
                            match Self::process_message(&coordinator, &message).await {
                                Some(response) => {
                                    if let Err(e) = transport.send_message(&response, Some(Protocol::Postcard), None).await {
                                        eprintln!("[ERROR] Edge-{}: Failed to send response: {}", edge_id, e);
                                        break;
                                    }
                                }
                                None => {
                                    eprintln!("[WARN] Edge-{}: No response for message from {:?} to {:?}",
                                        edge_id, message.header.source, message.header.target);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("[ERROR] Edge-{}: Message receive error: {}", edge_id, e);
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                    // Periodic connection check
                    continue;
                }
            }
        }

        // Cleanup
        if let Some(node_id) = device_node_id {
            let mut coordinator = coordinator.lock().await;
            coordinator.remove_service(node_id);
            println!(
                "[EDGE-{}] Device disconnected: {:?} (processed {} messages)",
                edge_id, node_id, message_count
            );
        }

        {
            let mut connections = connections.lock().await;
            connections.remove(&addr);
        }

        Ok(())
    }

    /// Add device service to coordinator
    async fn add_device_service(
        coordinator: &SharedCoordinator,
        message: &Message,
        max_devices: usize,
    ) -> Result<NodeId, &'static str> {
        let mut coordinator = coordinator.lock().await;

        let current_count = coordinator.service_count();
        if current_count >= max_devices {
            return Err("Max devices reached");
        }

        let device_node_id = message.header.source;

        // Check if already exists
        if coordinator.get_service_immutable(device_node_id).is_some() {
            return Ok(device_node_id);
        }

        // Create device service
        let time_provider = EdgeTimeProvider::new();
        let device_mac = match device_node_id {
            NodeId::Device(mac) => mac,
            _ => [0xDE, 0xDC, 0xCE, 0x00, 0x00, 0x01],
        };
        let uuid_generator = DeviceBasedUuidGenerator::new(device_mac);

        let sync_config = SyncConfig {
            sync_interval_ms: 10000,
            max_drift_ms: 50,
            offset_history_size: 3,
            delay_threshold_ms: 30,
            max_retry_count: 2,
            failure_cooldown_ms: 10000,
        };

        let device_service =
            TimeSyncService::new(time_provider, device_node_id, sync_config, uuid_generator);

        coordinator.add_service(device_node_id, device_service);
        Ok(device_node_id)
    }

    /// Process message through coordinator
    async fn process_message(
        coordinator: &SharedCoordinator,
        message: &Message,
    ) -> Option<Message> {
        let mut coordinator = coordinator.lock().await;
        coordinator.handle_time_sync_message(message)
    }

    /// Start cloud synchronization
    async fn start_cloud_sync(&self) {
        let coordinator = Arc::clone(&self.coordinator);
        let cloud_addr = self.config.cloud_addr.clone();
        let edge_id = self.config.edge_id;
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            while is_running.load(Ordering::SeqCst) {
                interval.tick().await;

                if let Err(e) = Self::sync_with_cloud(&coordinator, &cloud_addr, edge_id).await {
                    eprintln!("[ERROR] Edge-{}: Cloud sync failed: {}", edge_id, e);
                } else {
                    println!("[EDGE-{}] Cloud synchronized", edge_id);
                }
            }
        });
    }

    /// Sync with cloud server
    async fn sync_with_cloud(
        coordinator: &SharedCoordinator,
        cloud_addr: &str,
        edge_id: u8,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Connect to cloud
        let stream = TcpStream::connect(cloud_addr).await?;
        let tcp_adapter = TcpAdapter::new(stream);
        let mut transport = AsyncMessageTransport::new(tcp_adapter)
            .with_default_protocol(Protocol::Postcard)
            .with_crc(true);

        let edge_node_id = NodeId::Edge(edge_id);

        // Create sync request through coordinator
        let request = {
            let mut coordinator = coordinator.lock().await;

            match coordinator.get_service(edge_node_id) {
                Some(edge_service) => edge_service.create_sync_request(NodeId::Cloud)?,
                None => {
                    return Err("Edge service not found".into());
                }
            }
        };

        // Send request and receive response
        transport
            .send_message(&request, Some(Protocol::Postcard), None)
            .await?;
        let (response, _protocol, _stream_id) = transport.receive_message::<Message>().await?;

        // Process response through coordinator
        {
            let mut coordinator = coordinator.lock().await;

            match coordinator.get_service(edge_node_id) {
                Some(edge_service) => {
                    edge_service.handle_sync_response(&response)?;
                }
                None => {
                    return Err("Edge service not found during response processing".into());
                }
            }
        }

        Ok(())
    }

    /// Start status monitoring
    async fn start_status_monitor(&self) {
        let coordinator = Arc::clone(&self.coordinator);
        let connections = Arc::clone(&self.connections);
        let is_running = Arc::clone(&self.is_running);
        let edge_id = self.config.edge_id;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(120));

            while is_running.load(Ordering::SeqCst) {
                interval.tick().await;

                let (service_count, connected_devices) = {
                    let coordinator = coordinator.lock().await;
                    let connections = connections.lock().await;
                    (coordinator.service_count(), connections.len())
                };

                let network_status = {
                    let coordinator = coordinator.lock().await;
                    coordinator.get_network_status()
                };

                println!(
                    "[STATUS] Edge-{}: {} services, {} devices, {}/{} synced ({:.0}%)",
                    edge_id,
                    service_count,
                    connected_devices,
                    network_status.synced_nodes,
                    network_status.total_nodes,
                    network_status.sync_ratio() * 100.0
                );
            }
        });
    }

    /// Shutdown
    async fn shutdown(&mut self) {
        println!("[EDGE-{}] Shutting down", self.config.edge_id);
        self.is_running.store(false, Ordering::SeqCst);

        let connections = self.connections.lock().await;
        if !connections.is_empty() {
            println!(
                "[EDGE-{}] Closed {} device connections",
                self.config.edge_id,
                connections.len()
            );
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = EdgeConfig::default();

    // Parse simple command line arguments
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--edge-id" | "-e" => {
                if i + 1 < args.len() {
                    config.edge_id = args[i + 1].parse().unwrap_or(1);
                    i += 1;
                }
            }
            "--cloud-addr" | "-c" => {
                if i + 1 < args.len() {
                    config.cloud_addr = args[i + 1].clone();
                    i += 1;
                }
            }
            "--device-port" | "-p" => {
                if i + 1 < args.len() {
                    config.device_port = args[i + 1].parse().unwrap_or(9090);
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!("Edge Node - Time Synchronization Coordinator");
                println!("Usage: {} [OPTIONS]", args[0]);
                println!();
                println!("Options:");
                println!("  --edge-id, -e <ID>       Edge node ID (default: 1)");
                println!("  --cloud-addr, -c <ADDR>  Cloud server address (default: 127.0.0.1:8080)");
                println!("  --device-port, -p <PORT> Device listen port (default: 9090)");
                println!("  --help, -h               Show help");
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    let mut edge_node = SimpleEdgeNode::new(config);
    edge_node.start().await
}
