pub mod platform;

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use reqwest;
use serde_json;

use platform::WifiBackend;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub use super::*;

#[derive(Debug, Default, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Ssid(pub String);

#[derive(Debug, Default, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Bssid(pub [u8; 6]);

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Channel(pub u16);

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum Band {
    GHz2,
    GHz5,
    GHz6,
    #[default]
    Unknown,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum Security {
    Open,
    Wep,
    WpaPersonal,
    Wpa2Personal,
    Wpa3Personal,
    WpaEnterprise,
    Wpa2Enterprise,
    Wpa3Enterprise,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioLink {
    pub band: Band,
    pub channel: Channel,
    pub freq_mhz: u32,
    pub rssi_dbm: i8,       // ‑127 … 0 dBm
    pub snr_db: Option<u8>, // optional
    pub last_seen: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPoint {
    pub bssid: Bssid,
    pub links: Vec<RadioLink>,
    pub vendor_oui: Option<u32>,  // extracted from MAC if known
    pub phy_type: Option<String>, // HT/VHT/HE/EHT …
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    pub ssid: Ssid,
    pub security: Security,
    pub access_points: Vec<AccessPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSnapshot {
    pub timestamp: OffsetDateTime,
    pub networks: Vec<Network>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub ssid: Ssid,
    pub security: Security,
    pub passphrase: Option<String>,
    pub created_at: OffsetDateTime,
    pub auto_connect: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub state: ConnState,
    pub ssid: Option<Ssid>,
    pub access_point: Option<Bssid>,
    pub ip_address: Option<IpAddr>,
    pub gateway: Option<IpAddr>,
    pub dns_servers: Vec<IpAddr>,
    pub speed_mbps: Option<u32>,
    pub since: Option<OffsetDateTime>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum ConnState {
    Connected,
    #[default]
    Disconnected,
    Authenticating,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiEntry {
    #[serde(flatten)]
    pub network: Network,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<Credentials>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Wifi {
    pub wifis: HashMap<Ssid, WifiEntry>,
    pub current_connection: Option<ConnectionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub credentials: Credentials,
    pub endpoint: String,
}

impl Device {
    pub async fn send_wifi_config(&self, router_credentials: &Credentials) -> Result<()> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| WifiError::Backend(format!("Failed to create HTTP client: {}", e)))?;

        let password = router_credentials.passphrase.clone().unwrap_or_default();
        let wifi_config = serde_json::json!({
            "ssid": router_credentials.ssid.0,
            "password": password
        });

        let response = client
            .post(format!("{}/config", self.endpoint))
            .json(&wifi_config)
            .send()
            .await
            .map_err(|e| WifiError::Backend(format!("Failed to send config to device: {}", e)))?;

        if !response.status().is_success() {
            return Err(WifiError::Backend(format!(
                "Device returned error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        Ok(())
    }
}

pub struct WifiState {
    pub backend: Box<dyn WifiBackend>,
    pub cache: Wifi,
}

impl WifiState {
    pub fn new() -> Self {
        Self {
            backend: Box::new(platform::Backend::new()),
            cache: Default::default(),
        }
    }

    pub async fn scan_wifis(&mut self) -> Result<Wifi> {
        let networks = self.backend.scan().await?;
        let profiles = self.backend.get_profiles().await?;

        let mut profiles: HashMap<Ssid, Credentials> = profiles
            .into_iter()
            .map(|cred| (cred.ssid.clone(), cred))
            .collect();

        let wifis = networks
            .into_iter()
            .map(|network| {
                let credential = profiles.remove(&network.ssid);
                (
                    network.ssid.clone(),
                    WifiEntry {
                        network,
                        credential,
                    },
                )
            })
            .collect();

        self.cache = Wifi {
            wifis,
            current_connection: self.backend.current_connection().await?,
        };

        Ok(self.cache.clone())
    }

    pub async fn register_device(
        &mut self,
        device: Device,
        router_credentials: &Credentials,
    ) -> Result<()> {
        // Cache current connection before connecting to device
        let original_connection = self.backend.current_connection().await?;

        // Connect to the device AP
        self.backend.connect(&device.credentials).await?;

        tokio::time::sleep(Duration::from_secs(3)).await;

        // Send router WiFi config to device and query data from device
        device.send_wifi_config(router_credentials).await?;

        // Disconnect from device AP
        self.backend.disconnect().await?;

        // Restore original connection if there was one
        self.restore_original_connection(original_connection)
            .await?;

        // Update cache with current connection after restoration
        self.cache.current_connection = self.backend.current_connection().await?;

        // TODO: Upload device info to cloud

        Ok(())
    }

    async fn restore_original_connection(
        &mut self,
        original_connection: Option<ConnectionInfo>,
    ) -> Result<()> {
        if let Some(conn_info) = original_connection {
            if let (Some(original_ssid), ConnState::Connected) = (&conn_info.ssid, &conn_info.state)
            {
                // First try to find credentials in cache
                let original_creds = self
                    .cache
                    .wifis
                    .get(original_ssid)
                    .and_then(|entry| entry.credential.as_ref());

                if let Some(creds) = original_creds {
                    // Found in cache, use it directly
                    self.backend.connect(creds).await?;
                } else {
                    // Not in cache, fallback to backend query
                    let saved_profiles = self.backend.get_profiles().await?;
                    if let Some(original_creds) =
                        saved_profiles.iter().find(|p| p.ssid == *original_ssid)
                    {
                        self.backend.connect(original_creds).await?;
                    } else {
                        return Err(WifiError::NotFound(format!(
                            "Could not find saved credentials for: {}",
                            original_ssid.0
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}
