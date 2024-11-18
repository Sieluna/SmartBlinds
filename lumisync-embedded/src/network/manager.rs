use core::net::Ipv4Addr;
use core::str;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use embassy_time::{Duration, Instant, Timer};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::storage::LocalStorage;

use super::{
    FramedTcpTransport, FramedTransport, RawTransport, TcpTransport, WifiController,
    WifiEncryption,
    dhcp::{DhcpConfig, DhcpServer},
};

const CONFIG_KEY: &str = "wifi_config";
const DEFAULT_CONFIG_TIMEOUT: Duration = Duration::from_secs(300);
const DEFAULT_MAX_ATTEMPTS: u8 = 2;
const DEFAULT_CONFIG_PORT: u16 = 8080;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WifiState {
    Idle,
    CheckingStorage,
    StartingAP,
    WaitingForConfig,
    ConnectingSTA { attempts: u8 },
    ConnectedSTA,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiConfig {
    pub ssid: String,
    pub password: String,
    pub encryption: WifiEncryption,
}

#[derive(Debug, Clone)]
pub enum ConfigMessage {
    GetConfig,
    SetConfig(WifiConfig),
    ConfigAck,
    Error(String),
}

pub struct WifiManager<S: LocalStorage, W: WifiController> {
    state: WifiState,
    storage: S,
    wifi_controller: W,
    config_server: Option<FramedTcpTransport>,
    ap_ssid: String,
    ap_password: String,
    max_attempts: u8,
    config_timeout: Duration,
    config_port: u16,
    state_start_time: Option<Instant>,
    received_config: Option<WifiConfig>,
    tcp_rx_buffer: Option<&'static mut [u8]>,
    tcp_tx_buffer: Option<&'static mut [u8]>,
    network_stack: Option<embassy_net::Stack<'static>>,
    dhcp_config: DhcpConfig,
    dhcp_server: Option<DhcpServer>,
    dhcp_rx_meta: Option<&'static mut [embassy_net::udp::PacketMetadata]>,
    dhcp_rx_buffer: Option<&'static mut [u8]>,
    dhcp_tx_meta: Option<&'static mut [embassy_net::udp::PacketMetadata]>,
    dhcp_tx_buffer: Option<&'static mut [u8]>,
}

impl<S: LocalStorage, W: WifiController> WifiManager<S, W> {
    pub fn new(storage: S, wifi_controller: W, ap_ssid: String, ap_password: String) -> Self {
        Self {
            state: WifiState::Idle,
            storage,
            wifi_controller,
            config_server: None,
            ap_ssid,
            ap_password,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            config_timeout: DEFAULT_CONFIG_TIMEOUT,
            config_port: DEFAULT_CONFIG_PORT,
            state_start_time: None,
            received_config: None,
            tcp_rx_buffer: None,
            tcp_tx_buffer: None,
            network_stack: None,
            dhcp_config: DhcpConfig::default(),
            dhcp_server: None,
            dhcp_rx_meta: None,
            dhcp_rx_buffer: None,
            dhcp_tx_meta: None,
            dhcp_tx_buffer: None,
        }
    }

    pub fn with_config_timeout(mut self, timeout_secs: u64) -> Self {
        self.config_timeout = Duration::from_secs(timeout_secs);
        self
    }

    pub fn with_max_attempts(mut self, attempts: u8) -> Self {
        self.max_attempts = attempts;
        self
    }

    pub fn with_config_port(mut self, port: u16) -> Self {
        self.config_port = port;
        self
    }

    pub fn with_tcp_buffers(
        mut self,
        rx_buffer: &'static mut [u8],
        tx_buffer: &'static mut [u8],
    ) -> Self {
        self.tcp_rx_buffer = Some(rx_buffer);
        self.tcp_tx_buffer = Some(tx_buffer);
        self
    }

    pub fn with_dhcp_config(mut self, config: DhcpConfig) -> Self {
        self.dhcp_config = config;
        self
    }

    pub fn with_network_stack(mut self, stack: embassy_net::Stack<'static>) -> Self {
        self.network_stack = Some(stack);
        self
    }

    pub fn state(&self) -> &WifiState {
        &self.state
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.state, WifiState::ConnectedSTA)
    }

    pub fn is_dhcp_enabled(&self) -> bool {
        self.dhcp_server.is_some()
    }

    pub fn dhcp_config(&self) -> &DhcpConfig {
        &self.dhcp_config
    }

    pub async fn tick(&mut self) -> Result<(), Error> {
        if let Some(ref mut dhcp_server) = self.dhcp_server {
            dhcp_server.process().await?;
        }

        match self.state {
            WifiState::Idle => {
                self.transition_to(WifiState::CheckingStorage).await?;
            }
            WifiState::CheckingStorage => {
                if let Some(config) = self.load_wifi_config().await? {
                    log::info!("Found cached WiFi config, attempting connection");
                    self.received_config = Some(config);
                    self.transition_to(WifiState::ConnectingSTA { attempts: 0 })
                        .await?;
                } else {
                    log::info!("No cached WiFi config, starting AP mode");
                    self.transition_to(WifiState::StartingAP).await?;
                }
            }
            WifiState::StartingAP => {
                self.start_ap_mode().await?;
                self.transition_to(WifiState::WaitingForConfig).await?;
            }
            WifiState::WaitingForConfig => {
                self.poll_config_server().await?;
                if let Some(config) = self.received_config.take() {
                    log::info!("Received WiFi config, stopping AP and attempting connection");
                    self.stop_ap_mode().await?;
                    self.received_config = Some(config);
                    self.transition_to(WifiState::ConnectingSTA { attempts: 0 })
                        .await?;
                } else if self.is_timeout() {
                    log::warn!("Config timeout, restarting AP");
                    self.transition_to(WifiState::StartingAP).await?;
                }
            }
            WifiState::ConnectingSTA { attempts } => {
                if let Some(config) = self.received_config.clone() {
                    if self.attempt_sta_connection(&config).await? {
                        log::info!("Successfully connected to WiFi");
                        self.save_wifi_config(&config).await?;
                        self.transition_to(WifiState::ConnectedSTA).await?;
                    } else if attempts + 1 >= self.max_attempts {
                        log::error!("Failed to connect after {} attempts", attempts + 1);
                        self.received_config = None;
                        self.transition_to(WifiState::StartingAP).await?;
                    } else {
                        log::warn!("Connection attempt {} failed, retrying", attempts + 1);
                        Timer::after(Duration::from_secs(2)).await;
                        self.transition_to(WifiState::ConnectingSTA {
                            attempts: attempts + 1,
                        })
                        .await?;
                    }
                } else {
                    self.transition_to(WifiState::Failed).await?;
                }
            }
            WifiState::ConnectedSTA => {
                if !self.wifi_controller.is_connected() {
                    log::warn!("WiFi connection lost, reconnecting");
                    self.transition_to(WifiState::ConnectingSTA { attempts: 0 })
                        .await?;
                }
            }
            WifiState::Failed => {
                Timer::after(Duration::from_secs(10)).await;
                self.transition_to(WifiState::CheckingStorage).await?;
            }
        }

        Ok(())
    }

    pub async fn restart_config(&mut self) -> Result<(), Error> {
        self.stop_ap_mode().await?;
        self.wifi_controller
            .disconnect()
            .await
            .map_err(|_| Error::NetworkError)?;
        self.received_config = None;
        self.transition_to(WifiState::StartingAP).await
    }

    pub async fn clear_config(&mut self) -> Result<(), Error> {
        self.storage
            .remove_item(CONFIG_KEY)
            .await
            .map_err(|_| Error::InitializationError)?;
        self.restart_config().await
    }

    async fn transition_to(&mut self, new_state: WifiState) -> Result<(), Error> {
        log::debug!("WiFi state transition: {:?} -> {:?}", self.state, new_state);
        self.state = new_state;
        self.state_start_time = Some(Instant::now());
        Ok(())
    }

    fn is_timeout(&self) -> bool {
        self.state_start_time
            .map(|start| Instant::now().saturating_duration_since(start) > self.config_timeout)
            .unwrap_or(false)
    }

    async fn load_wifi_config(&mut self) -> Result<Option<WifiConfig>, Error> {
        let data = match self.storage.get_item(CONFIG_KEY).await {
            Ok(Some(data)) => data,
            _ => return Ok(None),
        };

        let mut parts = data.splitn(3, '|');
        let (ssid, password, enc_str) = match (parts.next(), parts.next(), parts.next()) {
            (Some(s), Some(p), Some(e)) => (s, p, e),
            _ => {
                log::warn!("Invalid WiFi config format in storage, clearing");
                let _ = self.storage.remove_item(CONFIG_KEY).await;
                return Ok(None);
            }
        };

        let encryption = match enc_str {
            "none" => WifiEncryption::None,
            "wep" => WifiEncryption::WEP,
            "wpa" => WifiEncryption::WPA,
            "wpa2" => WifiEncryption::WPA2,
            "wpa3" => WifiEncryption::WPA3,
            _ => {
                log::warn!("Unknown encryption type: {}, using WPA2", enc_str);
                WifiEncryption::WPA2
            }
        };

        Ok(Some(WifiConfig {
            ssid: ssid.to_string(),
            password: password.to_string(),
            encryption,
        }))
    }

    async fn save_wifi_config(&mut self, config: &WifiConfig) -> Result<(), Error> {
        let enc_str = match config.encryption {
            WifiEncryption::None => "none",
            WifiEncryption::WEP => "wep",
            WifiEncryption::WPA => "wpa",
            WifiEncryption::WPA2 => "wpa2",
            WifiEncryption::WPA3 => "wpa3",
        };

        let data = alloc::format!("{}|{}|{}", config.ssid, config.password, enc_str);

        self.storage
            .set_item(CONFIG_KEY, &data)
            .await
            .map_err(|_| Error::InitializationError)
    }

    async fn start_ap_mode(&mut self) -> Result<(), Error> {
        self.wifi_controller
            .start_ap(&self.ap_ssid, &self.ap_password)
            .await
            .map_err(|_| Error::NetworkError)?;

        self.start_dhcp_server().await?;
        self.start_config_server().await?;

        log::info!(
            "AP mode started: {} (IP: {}, DHCP: {}-{}, Config port: {})",
            self.ap_ssid,
            self.dhcp_config.server_ip,
            self.dhcp_config.pool_start,
            self.dhcp_config.pool_end,
            self.config_port
        );
        Ok(())
    }

    async fn stop_ap_mode(&mut self) -> Result<(), Error> {
        self.wifi_controller
            .stop_ap()
            .await
            .map_err(|_| Error::NetworkError)?;

        self.stop_dhcp_server().await?;

        if let Some(ref mut server) = self.config_server {
            server.inner_mut().close();
        }
        self.config_server = None;

        log::info!("AP mode stopped");
        Ok(())
    }

    async fn attempt_sta_connection(&mut self, config: &WifiConfig) -> Result<bool, Error> {
        log::info!("Attempting to connect to WiFi: {}", config.ssid);

        match self
            .wifi_controller
            .connect_station(&config.ssid, &config.password, config.encryption.clone())
            .await
        {
            Ok(()) => {
                Timer::after(Duration::from_secs(3)).await;
                Ok(self.wifi_controller.is_connected())
            }
            Err(_) => Ok(false),
        }
    }

    async fn start_dhcp_server(&mut self) -> Result<(), Error> {
        if let (Some(stack), Some(rx_meta), Some(rx_buffer), Some(tx_meta), Some(tx_buffer)) = (
            &self.network_stack,
            self.dhcp_rx_meta.take(),
            self.dhcp_rx_buffer.take(),
            self.dhcp_tx_meta.take(),
            self.dhcp_tx_buffer.take(),
        ) {
            let dhcp_server = DhcpServer::new(
                stack.clone(),
                self.dhcp_config.clone(),
                rx_meta,
                rx_buffer,
                tx_meta,
                tx_buffer,
            )?;

            self.dhcp_server = Some(dhcp_server);
            log::info!("DHCP server started on {}", self.dhcp_config.server_ip);
        } else {
            log::warn!("Cannot start DHCP server: missing network stack or buffers");
        }

        Ok(())
    }

    async fn stop_dhcp_server(&mut self) -> Result<(), Error> {
        if let Some(_dhcp_server) = self.dhcp_server.take() {
            log::info!("DHCP server stopped");
        }
        Ok(())
    }

    pub fn get_active_leases(&self) -> Vec<(Ipv4Addr, [u8; 6])> {
        if let Some(ref dhcp_server) = self.dhcp_server {
            dhcp_server.get_active_leases()
        } else {
            Vec::new()
        }
    }

    pub fn allocate_ip(&mut self, _mac_address: [u8; 6]) -> Option<Ipv4Addr> {
        if self.is_dhcp_enabled() {
            Some(self.dhcp_config.pool_start)
        } else {
            None
        }
    }

    pub fn get_dhcp_options(&self) -> Option<(Ipv4Addr, Ipv4Addr, Ipv4Addr, u32)> {
        if self.is_dhcp_enabled() {
            Some((
                self.dhcp_config.subnet_mask,
                self.dhcp_config.gateway,
                self.dhcp_config.dns_server,
                self.dhcp_config.lease_time,
            ))
        } else {
            None
        }
    }

    async fn start_config_server(&mut self) -> Result<(), Error> {
        if let (Some(stack), Some(rx_buffer), Some(tx_buffer)) = (
            &self.network_stack,
            self.tcp_rx_buffer.take(),
            self.tcp_tx_buffer.take(),
        ) {
            let tcp_transport = TcpTransport::new(stack.clone(), rx_buffer, tx_buffer);
            let framed = FramedTransport::new(tcp_transport);
            self.config_server = Some(framed);

            if let Some(ref mut server) = self.config_server {
                let endpoint = embassy_net::IpListenEndpoint {
                    addr: None,
                    port: self.config_port,
                };
                server.inner_mut().accept(endpoint).await?;
            }

            log::info!("TCP config server started on port {}", self.config_port);
        } else {
            log::warn!("Cannot start TCP server: missing network stack or buffers");
        }

        Ok(())
    }

    async fn poll_config_server(&mut self) -> Result<(), Error> {
        if let Some(server) = self.config_server.as_mut() {
            if server.inner().is_connected() {
                let mut buffer = [0u8; 512];
                if let Some(len) = server.receive_bytes(&mut buffer).await? {
                    if let Ok(message) = Self::parse_config_message(&buffer[..len]) {
                        let mut temp_server = self.config_server.take().unwrap();
                        self.handle_config_message(message, &mut temp_server)
                            .await?;
                        self.config_server = Some(temp_server);
                    }
                }
            }
        }
        Ok(())
    }

    fn parse_config_message(data: &[u8]) -> Result<ConfigMessage, Error> {
        let msg_str = str::from_utf8(data).map_err(|_| Error::SerializationError)?;
        let mut parts = msg_str.splitn(4, '|');

        match parts.next() {
            Some("GET_CONFIG") => Ok(ConfigMessage::GetConfig),
            Some("SET_CONFIG") => {
                let (ssid, password, enc_str) = match (parts.next(), parts.next(), parts.next()) {
                    (Some(s), Some(p), Some(e)) => (s, p, e),
                    _ => return Err(Error::SerializationError),
                };

                let encryption = match enc_str {
                    "none" => WifiEncryption::None,
                    "wep" => WifiEncryption::WEP,
                    "wpa" => WifiEncryption::WPA,
                    "wpa2" => WifiEncryption::WPA2,
                    "wpa3" => WifiEncryption::WPA3,
                    _ => return Err(Error::SerializationError),
                };

                Ok(ConfigMessage::SetConfig(WifiConfig {
                    ssid: ssid.to_string(),
                    password: password.to_string(),
                    encryption,
                }))
            }
            _ => Err(Error::SerializationError),
        }
    }

    fn serialize_config_message(message: &ConfigMessage) -> Vec<u8> {
        match message {
            ConfigMessage::GetConfig => b"GET_CONFIG".to_vec(),
            ConfigMessage::SetConfig(config) => {
                let enc_str = match config.encryption {
                    WifiEncryption::None => "none",
                    WifiEncryption::WEP => "wep",
                    WifiEncryption::WPA => "wpa",
                    WifiEncryption::WPA2 => "wpa2",
                    WifiEncryption::WPA3 => "wpa3",
                };
                alloc::format!("SET_CONFIG|{}|{}|{}", config.ssid, config.password, enc_str)
                    .into_bytes()
            }
            ConfigMessage::ConfigAck => b"CONFIG_ACK".to_vec(),
            ConfigMessage::Error(msg) => alloc::format!("ERROR|{}", msg).into_bytes(),
        }
    }

    async fn handle_config_message(
        &mut self,
        message: ConfigMessage,
        server: &mut FramedTcpTransport,
    ) -> Result<(), Error> {
        match message {
            ConfigMessage::GetConfig => {
                server
                    .send_bytes(&Self::serialize_config_message(&ConfigMessage::ConfigAck))
                    .await?;
                log::info!("Received config request, sent ACK");
            }
            ConfigMessage::SetConfig(config) => {
                let enc_type = match config.encryption {
                    WifiEncryption::None => "Open",
                    WifiEncryption::WEP => "WEP",
                    WifiEncryption::WPA => "WPA",
                    WifiEncryption::WPA2 => "WPA2",
                    WifiEncryption::WPA3 => "WPA3",
                };
                log::info!("Received WiFi config: {} ({})", config.ssid, enc_type);

                self.received_config = Some(config);
                server
                    .send_bytes(&Self::serialize_config_message(&ConfigMessage::ConfigAck))
                    .await?;
                log::info!("Config received, sent ACK");
            }
            _ => {}
        }

        Ok(())
    }

    pub fn provide_config(&mut self, config: WifiConfig) {
        if matches!(self.state, WifiState::WaitingForConfig) {
            self.received_config = Some(config);
        }
    }

    pub fn config_port(&self) -> u16 {
        self.config_port
    }

    pub fn ap_ssid(&self) -> &str {
        &self.ap_ssid
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use tokio::time::sleep;

    use super::*;

    type TestManager = WifiManager<MockStorage, MockWifiController>;

    #[derive(Debug, Default)]
    struct MockStorage {
        data: BTreeMap<String, String>,
    }

    impl LocalStorage for MockStorage {
        type Error = ();

        async fn get_item(&self, key: &str) -> Result<Option<String>, Self::Error> {
            Ok(self.data.get(key).cloned())
        }

        async fn set_item(&mut self, key: &str, value: &str) -> Result<(), Self::Error> {
            self.data.insert(key.to_string(), value.to_string());
            Ok(())
        }

        async fn remove_item(&mut self, key: &str) -> Result<(), Self::Error> {
            self.data.remove(key);
            Ok(())
        }

        async fn clear(&mut self) -> Result<(), Self::Error> {
            self.data.clear();
            Ok(())
        }
    }

    #[derive(Debug)]
    struct MockWifiController {
        connected: bool,
        ap_started: bool,
        connect_failure_count: u8,
        max_failures: u8,
    }

    impl MockWifiController {
        fn new() -> Self {
            Self {
                connected: false,
                ap_started: false,
                connect_failure_count: 0,
                max_failures: 0,
            }
        }

        fn simulate_disconnect(&mut self) {
            self.connected = false;
        }
    }

    impl WifiController for MockWifiController {
        type Error = ();

        async fn start_ap(&mut self, _ssid: &str, _password: &str) -> Result<(), Self::Error> {
            self.ap_started = true;
            self.connected = false;
            Ok(())
        }

        async fn stop_ap(&mut self) -> Result<(), Self::Error> {
            self.ap_started = false;
            Ok(())
        }

        async fn connect_station(
            &mut self,
            _ssid: &str,
            _password: &str,
            _encryption: WifiEncryption,
        ) -> Result<(), Self::Error> {
            if self.connect_failure_count < self.max_failures {
                self.connect_failure_count += 1;
                Err(())
            } else {
                sleep(tokio::time::Duration::from_millis(50)).await;
                self.connected = true;
                self.ap_started = false;
                Ok(())
            }
        }

        async fn disconnect(&mut self) -> Result<(), Self::Error> {
            self.connected = false;
            Ok(())
        }

        fn is_connected(&self) -> bool {
            self.connected
        }
    }

    async fn run_to_state(
        manager: &mut TestManager,
        target_state: WifiState,
        max_ticks: u8,
    ) -> bool {
        for _ in 0..max_ticks {
            manager.tick().await.unwrap();

            if manager.state() == &target_state {
                return true;
            }

            sleep(tokio::time::Duration::from_millis(10)).await;
        }
        false
    }

    #[tokio::test]
    async fn test_lifecycle() {
        let mut manager = WifiManager::new(
            MockStorage::default(),
            MockWifiController::new(),
            "TestAP".to_string(),
            "testpass".to_string(),
        )
        .with_config_timeout(1)
        .with_max_attempts(2);

        assert_eq!(manager.state(), &WifiState::Idle);
        assert!(!manager.is_dhcp_enabled());

        assert!(run_to_state(&mut manager, WifiState::WaitingForConfig, 5).await);
        assert!(manager.wifi_controller.ap_started);

        let msg = TestManager::parse_config_message(b"SET_CONFIG|HomeWiFi|secret123|wpa2").unwrap();

        if let ConfigMessage::SetConfig(config) = msg {
            assert_eq!(config.ssid, "HomeWiFi");
            assert_eq!(config.encryption, WifiEncryption::WPA2);

            manager.provide_config(config.clone());

            let serialized =
                TestManager::serialize_config_message(&ConfigMessage::SetConfig(config));

            assert!(serialized.starts_with(b"SET_CONFIG"));
        }

        assert!(run_to_state(&mut manager, WifiState::ConnectedSTA, 5).await);

        assert!(!manager.wifi_controller.ap_started);
        assert!(!manager.is_dhcp_enabled());
        assert!(manager.wifi_controller.is_connected());

        let saved = manager.storage.get_item(CONFIG_KEY).await.unwrap().unwrap();

        assert_eq!(saved, "HomeWiFi|secret123|wpa2");

        manager.wifi_controller.simulate_disconnect();
        manager.tick().await.unwrap();

        assert!(matches!(
            manager.state(),
            WifiState::ConnectingSTA { attempts: 0 }
        ));
    }
}
