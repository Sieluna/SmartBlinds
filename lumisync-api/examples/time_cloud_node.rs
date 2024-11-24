use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use lumisync_api::message::{Message, MessagePayload, NodeId, TimeSyncPayload};
use lumisync_api::time::{SyncConfig, TimeProvider, TimeSyncService};
use lumisync_api::transport::{AsyncMessageTransport, Protocol};
use lumisync_api::uuid::RandomUuidGenerator;
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::signal;

type DeviceConnections = Arc<Mutex<HashMap<String, Instant>>>;
type SharedMetrics = Arc<Mutex<CloudMetrics>>;
type RunningFlag = Arc<AtomicBool>;

#[derive(Clone)]
pub struct CloudTimeProvider;

impl TimeProvider for CloudTimeProvider {
    fn monotonic_time_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    fn wall_clock_time(&self) -> Option<OffsetDateTime> {
        Some(OffsetDateTime::now_utc())
    }

    fn has_authoritative_time(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
pub struct CloudNodeConfig {
    pub listen_port: u16,
    pub max_connections: usize,
    pub sync_config: SyncConfig,
}

impl Default for CloudNodeConfig {
    fn default() -> Self {
        Self {
            listen_port: 8080,
            max_connections: 100,
            sync_config: SyncConfig {
                sync_interval_ms: 30000,
                max_drift_ms: 1000,
                offset_history_size: 10,
                delay_threshold_ms: 100,
                max_retry_count: 3,
                failure_cooldown_ms: 60000,
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CloudMetrics {
    pub total_sync_requests: u64,
    pub successful_syncs: u64,
    pub failed_syncs: u64,
    pub active_connections: u64,
}

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

pub struct CloudNode {
    config: CloudNodeConfig,
    active_connections: DeviceConnections,
    metrics: SharedMetrics,
    is_running: RunningFlag,
}

impl CloudNode {
    pub fn new(config: CloudNodeConfig) -> Self {
        Self {
            config,
            active_connections: Arc::new(Mutex::new(HashMap::new())),
            metrics: Arc::new(Mutex::new(CloudMetrics::default())),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.is_running.store(true, Ordering::SeqCst);

        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.config.listen_port)).await?;
        println!("Cloud node listening on port {}", self.config.listen_port);

        self.start_metrics_task().await;

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            println!("New connection from: {}", addr);
                            let connections = Arc::clone(&self.active_connections);
                            let metrics = Arc::clone(&self.metrics);
                            let sync_config = self.config.sync_config.clone();
                            let is_running = Arc::clone(&self.is_running);

                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_connection(
                                    stream,
                                    addr.to_string(),
                                    connections,
                                    metrics,
                                    sync_config,
                                    is_running,
                                )
                                .await
                                {
                                    eprintln!("Connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => eprintln!("Failed to accept connection: {}", e),
                    }
                }
                _ = signal::ctrl_c() => {
                    println!("Received shutdown signal, stopping cloud node...");
                    break;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                    if !self.is_running.load(Ordering::SeqCst) {
                        break;
                    }
                }
            }
        }

        self.shutdown().await;
        Ok(())
    }

    async fn handle_connection(
        stream: TcpStream,
        addr: String,
        connections: DeviceConnections,
        metrics: SharedMetrics,
        sync_config: SyncConfig,
        is_running: RunningFlag,
    ) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut conns = connections.lock().await;
            conns.insert(addr.clone(), Instant::now());
        }

        let tcp_adapter = TcpAdapter::new(stream);
        let mut transport = AsyncMessageTransport::new(tcp_adapter)
            .with_default_protocol(Protocol::Postcard)
            .with_crc(true);

        let time_provider = CloudTimeProvider;
        let uuid_generator = RandomUuidGenerator;
        let mut time_service =
            TimeSyncService::new(time_provider, NodeId::Cloud, sync_config, uuid_generator);

        while is_running.load(Ordering::SeqCst) {
            tokio::select! {
                result = transport.receive_message::<Message>() => {
                    match result {
                        Ok((message, _protocol, _stream_id)) => {
                            {
                                let mut conns = connections.lock().await;
                                conns.insert(addr.clone(), Instant::now());
                            }

                            if let Some(response) = Self::process_message(&mut time_service, &message) {
                                if transport
                                    .send_message(&response, Some(Protocol::Postcard), None)
                                    .await
                                    .is_err()
                                {
                                    break;
                                }

                                {
                                    let mut metrics = metrics.lock().await;
                                    metrics.successful_syncs += 1;
                                    metrics.total_sync_requests += 1;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => continue,
            }
        }

        {
            let mut conns = connections.lock().await;
            conns.remove(&addr);
        }

        Ok(())
    }

    fn process_message(
        time_service: &mut TimeSyncService<CloudTimeProvider, RandomUuidGenerator>,
        message: &Message,
    ) -> Option<Message> {
        match &message.payload {
            MessagePayload::TimeSync(TimeSyncPayload::Request { .. }) => {
                time_service.handle_sync_request(message).ok()
            }
            MessagePayload::TimeSync(TimeSyncPayload::StatusQuery) => {
                time_service.handle_status_query(message).ok()
            }
            _ => None,
        }
    }

    async fn start_metrics_task(&self) {
        let metrics = Arc::clone(&self.metrics);
        let connections = Arc::clone(&self.active_connections);
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

            while is_running.load(Ordering::SeqCst) {
                interval.tick().await;

                let active_count = {
                    let conns = connections.lock().await;
                    conns.len() as u64
                };

                {
                    let mut metrics = metrics.lock().await;
                    metrics.active_connections = active_count;
                }

                println!("Active connections: {}", active_count);
            }
        });
    }

    pub async fn get_metrics(&self) -> CloudMetrics {
        self.metrics.lock().await.clone()
    }

    async fn shutdown(&mut self) {
        println!("Shutting down cloud node...");
        self.is_running.store(false, Ordering::SeqCst);

        let connections = self.active_connections.lock().await;
        println!("Closing {} active connections", connections.len());
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = CloudNodeConfig::default();

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" | "-p" => {
                if i + 1 < args.len() {
                    config.listen_port = args[i + 1].parse().unwrap_or_else(|_| {
                        eprintln!("Error: Invalid port number '{}'", args[i + 1]);
                        std::process::exit(1);
                    });
                    i += 1;
                } else {
                    eprintln!("Error: --port requires a value");
                    std::process::exit(1);
                }
            }
            "--help" | "-h" => {
                println!("Time Cloud Node");
                println!("Usage: {} [OPTIONS]", args[0]);
                println!();
                println!("Options:");
                println!("  --port, -p <PORT>    Set listen port (default: 8080)");
                println!("  --help, -h           Show this help message");
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

    let mut cloud_node = CloudNode::new(config.clone());

    println!("Starting cloud node on port {}", config.listen_port);
    println!("Providing authoritative time synchronization");

    cloud_node.start().await
}
