use crate::configs::Storage;
use crate::errors::ApiError;
use crate::services::handlers::{
    AnalyticsHandler, CloudTimeSyncHandler, CommandDispatcher, DeviceStatusHandler,
};
use anyhow;
use lumisync_api::adapter::{AdapterManager, TransportType};
use lumisync_api::router::{BaseMessageRouter, MessageRouter, RouterConfig};
use lumisync_api::time::{TimeProvider, TimeSyncCoordinator};
use lumisync_api::uuid::RandomUuidGenerator;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use time::OffsetDateTime;
use tokio::sync::{RwLock, oneshot};
use tracing::{error, info, warn};

/// System time provider implementation
#[derive(Debug, Default, Clone)]
pub struct SystemTimeProvider;

impl TimeProvider for SystemTimeProvider {
    fn monotonic_time_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn wall_clock_time(&self) -> Option<OffsetDateTime> {
        Some(OffsetDateTime::now_utc())
    }

    fn has_authoritative_time(&self) -> bool {
        true
    }
}

type CloudTimeSyncCoordinator = TimeSyncCoordinator<SystemTimeProvider, RandomUuidGenerator>;

/// Cloud message service for handling distributed messaging
pub struct MessageService {
    router: Arc<RwLock<BaseMessageRouter>>,
    adapter_manager: Arc<tokio::sync::Mutex<AdapterManager>>,
    time_coordinator: Arc<RwLock<CloudTimeSyncCoordinator>>,
    storage: Arc<Storage>,
    config: ServiceConfig,
    is_running: Arc<RwLock<bool>>,
    stop_tx: Option<oneshot::Sender<()>>,
}

/// Service configuration parameters
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// List of authorized edge nodes
    pub authorized_edges: HashSet<u8>,
    /// Device routing table (device_id -> edge_id)
    pub device_routing: BTreeMap<i32, u8>,
    /// Enabled transport types
    pub enabled_transports: Vec<TransportType>,
    /// Message processing timeout in milliseconds
    pub message_timeout_ms: u64,
    /// Message processing interval in milliseconds
    pub processing_interval_ms: u64,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            authorized_edges: HashSet::new(),
            device_routing: BTreeMap::new(),
            enabled_transports: vec![TransportType::Tcp, TransportType::WebSocket],
            message_timeout_ms: 5000,
            processing_interval_ms: 10,
        }
    }
}

impl MessageService {
    /// Create a new message service instance
    pub async fn new(storage: Arc<Storage>, config: ServiceConfig) -> Result<Self, ApiError> {
        // Create router configuration
        let router_config = RouterConfig {
            max_handlers: 32,
            message_timeout_ms: config.message_timeout_ms,
            enable_duplicate_detection: true,
            duplicate_window_size: 1000,
            enable_stats: true,
            stats_retention_ms: 3600000, // 1 hour
        };

        // Create message router
        let router = BaseMessageRouter::new(router_config);

        // Create transport adapter manager
        let adapter_manager = Arc::new(tokio::sync::Mutex::new(AdapterManager::new()));

        // Create time synchronization coordinator
        let time_coordinator = Arc::new(RwLock::new(CloudTimeSyncCoordinator::new()));

        let service = Self {
            router: Arc::new(RwLock::new(router)),
            adapter_manager,
            time_coordinator,
            storage,
            config,
            is_running: Arc::new(RwLock::new(false)),
            stop_tx: None,
        };

        // Register message handlers
        service.register_handlers().await?;

        Ok(service)
    }

    /// Register all message handlers
    async fn register_handlers(&self) -> Result<(), ApiError> {
        let mut router = self.router.write().await;

        // Register time sync handler
        let time_sync_handler = CloudTimeSyncHandler::new(
            self.time_coordinator.clone(),
            self.config.authorized_edges.clone(),
            Some(self.storage.clone()),
        );

        let handler_id = router
            .register_handler(Box::new(time_sync_handler))
            .map_err(|e| {
                ApiError::InternalError(anyhow::anyhow!(
                    "Failed to register time sync handler: {}",
                    e
                ))
            })?;
        info!(
            "Time sync handler registered successfully, ID: {}",
            handler_id
        );

        // Register device status handler
        let device_status_handler = DeviceStatusHandler::new(self.storage.clone());

        let handler_id = router
            .register_handler(Box::new(device_status_handler))
            .map_err(|e| {
                ApiError::InternalError(anyhow::anyhow!(
                    "Failed to register device status handler: {}",
                    e
                ))
            })?;
        info!(
            "Device status handler registered successfully, ID: {}",
            handler_id
        );

        // Register command dispatcher
        let command_dispatcher = CommandDispatcher::new(
            self.adapter_manager.clone(),
            self.config.device_routing.clone(),
            Some(self.storage.clone()),
        );

        let handler_id = router
            .register_handler(Box::new(command_dispatcher))
            .map_err(|e| {
                ApiError::InternalError(anyhow::anyhow!(
                    "Failed to register command dispatcher: {}",
                    e
                ))
            })?;
        info!(
            "Command dispatcher registered successfully, ID: {}",
            handler_id
        );

        // Register analytics handler
        let analytics_handler = AnalyticsHandler::new(self.storage.clone());

        let handler_id = router
            .register_handler(Box::new(analytics_handler))
            .map_err(|e| {
                ApiError::InternalError(anyhow::anyhow!(
                    "Failed to register analytics handler: {}",
                    e
                ))
            })?;
        info!(
            "Analytics handler registered successfully, ID: {}",
            handler_id
        );

        Ok(())
    }

    /// Start the message service
    pub async fn start(&mut self) -> Result<(), ApiError> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Err(ApiError::InternalError(anyhow::anyhow!(
                "Service is already running"
            )));
        }

        info!("Starting message service...");

        // Start router
        {
            let mut router = self.router.write().await;
            router.start().map_err(|e| {
                ApiError::InternalError(anyhow::anyhow!("Failed to start router: {}", e))
            })?;
        }

        // Create stop signal channel
        let (stop_tx, stop_rx) = oneshot::channel();
        self.stop_tx = Some(stop_tx);

        // Clone necessary references for the background task
        let router = self.router.clone();
        let adapter_manager = self.adapter_manager.clone();
        let processing_interval = self.config.processing_interval_ms;

        // Start main message processing loop in background
        tokio::spawn(async move {
            Self::run_message_loop(router, adapter_manager, stop_rx, processing_interval).await;
        });

        *is_running = true;
        info!("Message service started successfully");

        Ok(())
    }

    /// Main message processing loop
    async fn run_message_loop(
        router: Arc<RwLock<BaseMessageRouter>>,
        adapter_manager: Arc<tokio::sync::Mutex<AdapterManager>>,
        mut stop_rx: oneshot::Receiver<()>,
        processing_interval_ms: u64,
    ) {
        let mut receive_interval =
            tokio::time::interval(tokio::time::Duration::from_millis(processing_interval_ms));

        loop {
            tokio::select! {
                _ = &mut stop_rx => {
                    info!("Received stop signal, exiting message processing loop");
                    break;
                }
                _ = receive_interval.tick() => {
                    // Receive messages from adapters
                    let message_opt = {
                        let mut adapter_mgr = adapter_manager.lock().await;
                        match adapter_mgr.try_receive_any() {
                            Ok(msg_opt) => msg_opt,
                            Err(e) => {
                                error!("Failed to receive message: {}", e);
                                continue;
                            }
                        }
                    };

                    // Process received messages
                    if let Some((_source_node, message)) = message_opt {
                        let mut router = router.write().await;
                        if let Err(e) = router.route_message(message) {
                            error!("Message routing failed: {}", e);
                        }
                    }
                }
            }
        }
    }

    /// Stop the message service
    pub async fn stop(&mut self) -> Result<(), ApiError> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            warn!("Service is already stopped");
            return Ok(());
        }

        info!("Stopping message service...");

        // Send stop signal
        if let Some(stop_tx) = self.stop_tx.take() {
            if stop_tx.send(()).is_err() {
                warn!("Failed to send stop signal - receiver may have been dropped");
            }
        }

        // Stop router
        {
            let mut router = self.router.write().await;
            router.stop().map_err(|e| {
                ApiError::InternalError(anyhow::anyhow!("Failed to stop router: {}", e))
            })?;
        }

        *is_running = false;
        info!("Message service stopped successfully");

        Ok(())
    }

    /// Get current service status
    pub async fn get_service_status(&self) -> ServiceStatus {
        let is_running = *self.is_running.read().await;
        let router_stats = {
            let router = self.router.read().await;
            router.get_stats()
        };

        let transport_stats = {
            let adapter_mgr = self.adapter_manager.lock().await;
            adapter_mgr.get_all_stats()
        };

        let time_sync_status = {
            let coordinator = self.time_coordinator.read().await;
            coordinator.get_network_status()
        };

        ServiceStatus {
            is_running,
            router_stats,
            transport_stats,
            time_sync_status,
            authorized_edges: self.config.authorized_edges.len(),
            managed_devices: self.config.device_routing.len(),
        }
    }

    /// Update device routing configuration
    pub async fn update_device_routing(
        &mut self,
        device_id: i32,
        edge_id: u8,
    ) -> Result<(), ApiError> {
        if !self.config.authorized_edges.contains(&edge_id) {
            return Err(ApiError::InternalError(anyhow::anyhow!(
                "Edge node {} is not authorized",
                edge_id
            )));
        }

        self.config.device_routing.insert(device_id, edge_id);
        info!(
            "Device {} routing updated to edge node {}",
            device_id, edge_id
        );
        Ok(())
    }

    /// Add authorized edge node
    pub async fn authorize_edge(&mut self, edge_id: u8) {
        if self.config.authorized_edges.insert(edge_id) {
            info!("Edge node {} has been authorized", edge_id);
        } else {
            warn!("Edge node {} was already authorized", edge_id);
        }
    }

    /// Remove edge node authorization
    pub async fn revoke_edge_authorization(&mut self, edge_id: u8) {
        if self.config.authorized_edges.remove(&edge_id) {
            info!("Authorization for edge node {} has been revoked", edge_id);

            // Remove all device routings to this edge
            self.config.device_routing.retain(|_, &mut v| v != edge_id);
        } else {
            warn!("Edge node {} was not authorized", edge_id);
        }
    }

    /// Check if an edge node is authorized
    pub fn is_edge_authorized(&self, edge_id: u8) -> bool {
        self.config.authorized_edges.contains(&edge_id)
    }

    /// Get device routing for a specific device
    pub fn get_device_routing(&self, device_id: i32) -> Option<u8> {
        self.config.device_routing.get(&device_id).copied()
    }
}

/// Service status information
#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub is_running: bool,
    pub router_stats: lumisync_api::router::RouterStats,
    pub transport_stats: BTreeMap<TransportType, lumisync_api::adapter::TransportStats>,
    pub time_sync_status: lumisync_api::time::NetworkStatus,
    pub authorized_edges: usize,
    pub managed_devices: usize,
}

#[cfg(test)]
mod tests {
    use crate::tests::setup_test_db;

    use super::*;

    #[tokio::test]
    async fn test_service_creation() {
        let storage = setup_test_db().await;
        let config = ServiceConfig::default();

        let service = MessageService::new(storage, config).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_service_lifecycle() {
        let storage = setup_test_db().await;
        let config = ServiceConfig::default();

        let mut service = MessageService::new(storage, config).await.unwrap();

        // Test starting and stopping
        assert!(service.start().await.is_ok());
        assert!(service.stop().await.is_ok());
    }

    #[tokio::test]
    async fn test_device_routing_update() {
        let storage = setup_test_db().await;
        let mut config = ServiceConfig::default();
        config.authorized_edges.insert(2);

        let mut service = MessageService::new(storage, config).await.unwrap();

        assert!(service.update_device_routing(1, 2).await.is_ok());
        assert_eq!(service.get_device_routing(1), Some(2));

        // Test unauthorized edge
        assert!(service.update_device_routing(2, 3).await.is_err());
    }

    #[tokio::test]
    async fn test_edge_authorization() {
        let storage = setup_test_db().await;
        let config = ServiceConfig::default();

        let mut service = MessageService::new(storage, config).await.unwrap();

        service.authorize_edge(1).await;
        assert!(service.is_edge_authorized(1));

        service.revoke_edge_authorization(1).await;
        assert!(!service.is_edge_authorized(1));
    }

    #[tokio::test]
    async fn test_edge_revocation_removes_device_routing() {
        let storage = setup_test_db().await;
        let mut config = ServiceConfig::default();
        config.authorized_edges.insert(1);

        let mut service = MessageService::new(storage, config).await.unwrap();

        // Add device routing
        service.update_device_routing(100, 1).await.unwrap();
        assert_eq!(service.get_device_routing(100), Some(1));

        // Revoke edge authorization
        service.revoke_edge_authorization(1).await;

        // Device routing should be removed
        assert_eq!(service.get_device_routing(100), None);
    }
}
