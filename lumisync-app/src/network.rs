use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::{Error, Result};
use crate::wifi::platform::WifiBackend;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub ssid: String,
    pub signal_strength: i8,
    pub security: String,
    pub frequency: Option<u32>,
    pub is_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkScanResult {
    pub networks: Vec<NetworkInfo>,
    pub current_connection: Option<String>,
    pub scan_timestamp: OffsetDateTime,
}

pub struct NetworkManager {
    backend: Box<dyn WifiBackend>,
}

impl NetworkManager {
    pub fn new() -> Self {
        Self {
            backend: Box::new(crate::wifi::platform::Backend::new()),
        }
    }

    /// Scan for available WiFi networks
    pub async fn scan_networks(&mut self) -> Result<NetworkScanResult> {
        // Scan networks
        let networks = self
            .backend
            .scan()
            .await
            .map_err(|e| Error::network(format!("Failed to scan networks: {:?}", e)))?;

        // Get current connection status
        let current_connection =
            self.backend.current_connection().await.map_err(|e| {
                Error::network(format!("Failed to get current connection: {:?}", e))
            })?;

        let current_ssid = current_connection
            .and_then(|conn| conn.ssid)
            .map(|ssid| ssid.0);

        // Convert to unified network information format
        let network_list = networks
            .into_iter()
            .map(|network| {
                let ssid = network.ssid.0.clone();
                let is_connected = current_ssid.as_ref() == Some(&ssid);
                let signal_strength = Self::calculate_signal_strength(&network);

                NetworkInfo {
                    ssid,
                    signal_strength,
                    security: format!("{:?}", network.security),
                    frequency: Self::extract_frequency(&network),
                    is_connected,
                }
            })
            .collect();

        Ok(NetworkScanResult {
            networks: network_list,
            current_connection: current_ssid,
            scan_timestamp: OffsetDateTime::now_utc(),
        })
    }

    /// Calculate network signal strength
    fn calculate_signal_strength(network: &crate::wifi::Network) -> i8 {
        network
            .access_points
            .iter()
            .flat_map(|ap| &ap.links)
            .map(|link| link.rssi_dbm)
            .max()
            .unwrap_or(-127)
    }

    /// Extract network frequency information
    fn extract_frequency(network: &crate::wifi::Network) -> Option<u32> {
        network
            .access_points
            .iter()
            .flat_map(|ap| &ap.links)
            .map(|link| link.freq_mhz)
            .next()
    }
}

impl Default for NetworkManager {
    fn default() -> Self {
        Self::new()
    }
}
