use std::collections::HashMap;
use std::time::Duration;

use reqwest;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::error::{DeviceErrorKind, Error, Result};
use crate::wifi::platform::WifiBackend;
use crate::wifi::{Credentials, Security, Ssid};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub ssid: String,
    pub endpoint: String,
    pub device_type: DeviceType,
    pub signal_strength: i8,
    pub security: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceType {
    SmartBlinds,
    EnvironmentalSensor,
    Esp32Device,
    GenericIoT,
}

impl DeviceType {
    fn from_ssid(ssid: &str) -> Self {
        if ssid.contains("Blinds") || ssid.contains("Window") {
            DeviceType::SmartBlinds
        } else if ssid.contains("Sensor") {
            DeviceType::EnvironmentalSensor
        } else if ssid.contains("ESP32") {
            DeviceType::Esp32Device
        } else {
            DeviceType::GenericIoT
        }
    }

    fn default_endpoint(&self) -> String {
        match self {
            DeviceType::SmartBlinds => "http://192.168.4.1:8080".to_string(),
            _ => "http://192.168.4.1:80".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfigRequest {
    pub ssid: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceDiscoveryResult {
    pub devices: Vec<DeviceInfo>,
    pub scan_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceAction {
    Discover,
    Configure {
        device_ssid: String,
        router_ssid: String,
        router_password: Option<String>,
    },
    Connect {
        ssid: String,
        password: String,
    },
    Disconnect,
    ListProfiles,
}

pub struct DeviceManager {
    backend: Box<dyn WifiBackend>,
    http_client: reqwest::Client,
    last_connection: Option<ConnectionState>,
    /// Cached network profiles for fast access
    cached_profiles: HashMap<String, Credentials>,
    /// Cache timestamp to know when to refresh
    cache_timestamp: Option<std::time::Instant>,
}

#[derive(Debug, Clone)]
struct ConnectionState {
    ssid: String,
    credentials: Option<Credentials>,
}

impl DeviceManager {
    pub fn new() -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self {
            backend: Box::new(crate::wifi::platform::Backend::new()),
            http_client,
            last_connection: None,
            cached_profiles: HashMap::new(),
            cache_timestamp: None,
        }
    }

    /// Execute device management actions
    pub async fn execute_action(&mut self, action: DeviceAction) -> Result<serde_json::Value> {
        match action {
            DeviceAction::Discover => {
                let result = self.discover_devices().await?;
                Ok(serde_json::to_value(result)?)
            }
            DeviceAction::Configure {
                device_ssid,
                router_ssid,
                router_password,
            } => {
                let password = match router_password {
                    Some(pwd) => pwd,
                    None => {
                        // Try to get cached password first, then fallback to saved profiles
                        self.get_cached_password(&router_ssid)
                            .await
                            .unwrap_or_default()
                    }
                };

                // Check if network actually needs a password before proceeding
                let needs_password = self.check_network_security(&router_ssid).await?;
                if needs_password && password.is_empty() {
                    return Err(Error::device(
                        format!(
                            "Network '{}' requires a password but none provided or cached",
                            router_ssid
                        ),
                        DeviceErrorKind::Wifi,
                    ));
                }

                self.configure_device(&device_ssid, &router_ssid, &password)
                    .await?;
                Ok(serde_json::json!({
                    "status": "configured",
                    "password_required": needs_password
                }))
            }
            DeviceAction::Connect { ssid, password } => {
                self.connect_to_network(&ssid, &password).await?;
                Ok(serde_json::json!({"status": "connected"}))
            }
            DeviceAction::Disconnect => {
                self.disconnect_from_network().await?;
                Ok(serde_json::json!({"status": "disconnected"}))
            }
            DeviceAction::ListProfiles => {
                // Refresh cache and return profiles
                self.refresh_profile_cache().await?;

                let profile_list: Vec<_> = self
                    .cached_profiles
                    .values()
                    .map(|profile| {
                        serde_json::json!({
                            "ssid": profile.ssid.0,
                            "security": format!("{:?}", profile.security),
                            "has_password": profile.passphrase.is_some(),
                            "auto_connect": profile.auto_connect,
                            "created_at": profile.created_at
                        })
                    })
                    .collect();

                Ok(serde_json::json!({
                    "profiles": profile_list,
                    "count": profile_list.len(),
                    "cached": true
                }))
            }
        }
    }

    /// Check if a network requires a password by scanning for it
    async fn check_network_security(&mut self, ssid: &str) -> Result<bool> {
        // Scan for networks to get security information
        let networks = self.backend.scan().await.map_err(|e| {
            Error::device(
                format!("Failed to scan for network security: {:?}", e),
                DeviceErrorKind::Wifi,
            )
        })?;

        // Find the target network and check its security
        for network in networks {
            if network.ssid.0 == ssid {
                return Ok(!matches!(network.security, Security::Open));
            }
        }

        // If not found in scan, assume it needs a password for safety
        Ok(true)
    }

    /// Get cached password for a network
    async fn get_cached_password(&mut self, ssid: &str) -> Option<String> {
        // Refresh cache if needed (every 5 minutes)
        let should_refresh = self
            .cache_timestamp
            .map_or(true, |ts| ts.elapsed() > Duration::from_secs(300));

        if should_refresh {
            if let Err(e) = self.refresh_profile_cache().await {
                eprintln!("Warning: Failed to refresh profile cache: {}", e);
            }
        }

        self.cached_profiles
            .get(ssid)
            .and_then(|creds| creds.passphrase.clone())
    }

    /// Refresh the profile cache
    async fn refresh_profile_cache(&mut self) -> Result<()> {
        let profiles = self.backend.get_profiles().await.map_err(|e| {
            Error::device(
                format!("Failed to get profiles for cache: {:?}", e),
                DeviceErrorKind::Wifi,
            )
        })?;

        self.cached_profiles.clear();
        for profile in profiles {
            self.cached_profiles.insert(profile.ssid.0.clone(), profile);
        }
        self.cache_timestamp = Some(std::time::Instant::now());

        Ok(())
    }

    /// Configure device with automatic connection restoration
    async fn configure_device(
        &mut self,
        device_ssid: &str,
        router_ssid: &str,
        router_password: &str,
    ) -> Result<()> {
        // 1. Save current connection state
        self.save_current_connection().await?;

        // 2. Get or create credentials for the router network
        let router_credentials = self
            .get_or_create_credentials(router_ssid, router_password)
            .await?;

        // 3. Connect to device AP
        self.connect_to_device_ap(device_ssid).await?;

        // 4. Wait for connection to stabilize
        tokio::time::sleep(Duration::from_secs(3)).await;

        // 5. Send configuration and ensure we always handle restoration
        let config_result = self
            .send_device_configuration(
                &router_credentials.ssid.0,
                router_credentials.passphrase.as_deref().unwrap_or(""),
                "192.168.4.1",
            )
            .await;

        // 6. Always attempt to restore previous connection
        let restore_result = self.restore_previous_connection().await;

        // 7. Check results
        config_result?;
        if let Err(e) = restore_result {
            // Log but don't fail - user can manually reconnect
            eprintln!("Warning: Failed to restore previous connection: {}", e);
        }

        // 8. Update cache with new credentials if they were created
        if !router_password.is_empty() {
            self.cached_profiles
                .insert(router_ssid.to_string(), router_credentials);
        }

        Ok(())
    }

    /// Save current connection state for later restoration
    async fn save_current_connection(&mut self) -> Result<()> {
        if let Some(connection_info) = self.backend.current_connection().await? {
            if let Some(ssid) = connection_info.ssid {
                // Try to get cached credentials first, then saved credentials
                let saved_credentials = self
                    .get_cached_password(&ssid.0)
                    .await
                    .and_then(|pwd| Some(self.create_credentials(&ssid.0, &pwd)))
                    .or_else(|| self.get_saved_credentials_sync(&ssid.0));

                self.last_connection = Some(ConnectionState {
                    ssid: ssid.0,
                    credentials: saved_credentials,
                });
            }
        }
        Ok(())
    }

    /// Get saved credentials synchronously from cache
    fn get_saved_credentials_sync(&self, ssid: &str) -> Option<Credentials> {
        self.cached_profiles.get(ssid).cloned()
    }

    /// Restore previous connection if available
    async fn restore_previous_connection(&mut self) -> Result<()> {
        if let Some(connection_state) = &self.last_connection {
            if let Some(credentials) = &connection_state.credentials {
                self.backend.connect(credentials).await.map_err(|e| {
                    Error::connection(format!(
                        "Failed to restore connection to {}: {:?}",
                        connection_state.ssid, e
                    ))
                })?;
            }
        }
        Ok(())
    }

    /// Get saved credentials for a network or create new ones
    async fn get_or_create_credentials(
        &mut self,
        ssid: &str,
        password: &str,
    ) -> Result<Credentials> {
        // First try to get cached credentials
        if let Some(cached_creds) = self.cached_profiles.get(ssid) {
            // If we have cached credentials and no new password provided, use cached
            if password.is_empty() && cached_creds.passphrase.is_some() {
                return Ok(cached_creds.clone());
            }
        }

        // Create new credentials with provided password
        Ok(self.create_credentials(ssid, password))
    }

    /// Create credentials object
    fn create_credentials(&self, ssid: &str, password: &str) -> Credentials {
        Credentials {
            ssid: Ssid(ssid.to_string()),
            security: if password.is_empty() {
                Security::Open
            } else {
                Security::Wpa2Personal
            },
            passphrase: if password.is_empty() {
                None
            } else {
                Some(password.to_string())
            },
            created_at: time::OffsetDateTime::now_utc(),
            auto_connect: true,
            hidden: false,
        }
    }

    /// Discover devices
    async fn discover_devices(&mut self) -> Result<DeviceDiscoveryResult> {
        let start_time = std::time::Instant::now();

        let networks = self.backend.scan().await.map_err(|e| {
            Error::device(
                format!("Failed to scan for devices: {:?}", e),
                DeviceErrorKind::Generic,
            )
        })?;

        let devices = networks
            .into_iter()
            .map(|network| {
                let device_type = DeviceType::from_ssid(&network.ssid.0);
                let signal_strength = Self::calculate_signal_strength(&network);

                DeviceInfo {
                    ssid: network.ssid.0.clone(),
                    endpoint: device_type.default_endpoint(),
                    device_type,
                    signal_strength,
                    security: format!("{:?}", network.security),
                }
            })
            .collect();

        Ok(DeviceDiscoveryResult {
            devices,
            scan_duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    /// Connect to specified network
    async fn connect_to_network(&mut self, ssid: &str, password: &str) -> Result<()> {
        let credentials = Credentials {
            ssid: Ssid(ssid.to_string()),
            security: if password.is_empty() {
                Security::Open
            } else {
                Security::Wpa2Personal
            },
            passphrase: if password.is_empty() {
                None
            } else {
                Some(password.to_string())
            },
            created_at: time::OffsetDateTime::now_utc(),
            auto_connect: true,
            hidden: false,
        };

        self.backend
            .connect(&credentials)
            .await
            .map_err(|e| Error::connection_to(format!("Failed to connect: {:?}", e), ssid))?;

        Ok(())
    }

    /// Disconnect from current network
    async fn disconnect_from_network(&mut self) -> Result<()> {
        self.backend
            .disconnect()
            .await
            .map_err(|e| Error::connection(format!("Failed to disconnect: {:?}", e)))?;

        Ok(())
    }

    /// Connect to device access point
    async fn connect_to_device_ap(&mut self, device_ssid: &str) -> Result<()> {
        let credentials = Credentials {
            ssid: Ssid(device_ssid.to_string()),
            security: Security::Open,
            passphrase: None,
            created_at: time::OffsetDateTime::now_utc(),
            auto_connect: false,
            hidden: false,
        };

        self.backend.connect(&credentials).await.map_err(|e| {
            Error::device(
                format!("Failed to connect to device {}: {:?}", device_ssid, e),
                DeviceErrorKind::Generic,
            )
        })?;

        Ok(())
    }

    /// Send WiFi configuration to device using TCP
    async fn send_device_configuration(
        &self,
        router_ssid: &str,
        router_password: &str,
        device_endpoint: &str,
    ) -> Result<()> {
        let config_message = format!("WIFI_CONFIG|{}|{}\n", router_ssid, router_password);

        let mut stream = timeout(
            Duration::from_secs(10),
            TcpStream::connect(&device_endpoint),
        )
        .await
        .map_err(|_| Error::connection_to("TCP connection timed out", device_endpoint))?
        .map_err(|e| {
            Error::device(
                format!("Failed to connect to device via TCP: {}", e),
                DeviceErrorKind::Generic,
            )
        })?;

        stream
            .write_all(config_message.as_bytes())
            .await
            .map_err(|e| {
                Error::device(
                    format!("Failed to send TCP configuration: {}", e),
                    DeviceErrorKind::Generic,
                )
            })?;

        let mut buffer = [0; 1024];
        let bytes_read = timeout(Duration::from_secs(10), stream.read(&mut buffer))
            .await
            .map_err(|_| Error::device("TCP response timeout", DeviceErrorKind::Generic))?
            .map_err(|e| {
                Error::device(
                    format!("Failed to read TCP response: {}", e),
                    DeviceErrorKind::Generic,
                )
            })?;

        let response = String::from_utf8_lossy(&buffer[..bytes_read]);

        if !response.starts_with("OK") {
            return Err(Error::device(
                format!("Device rejected TCP configuration: {}", response),
                DeviceErrorKind::Generic,
            ));
        }

        Ok(())
    }

    /// Calculate device signal strength
    fn calculate_signal_strength(network: &crate::wifi::Network) -> i8 {
        network
            .access_points
            .iter()
            .flat_map(|ap| &ap.links)
            .map(|link| link.rssi_dbm)
            .max()
            .unwrap_or(-127)
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}
