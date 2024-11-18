#[cfg(feature = "udp")]
mod dhcp;
#[cfg(all(feature = "tcp", feature = "udp"))]
mod manager;
mod transport;

#[cfg(feature = "udp")]
pub use dhcp::*;
#[cfg(all(feature = "tcp", feature = "udp"))]
pub use manager::*;
pub use transport::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WifiEncryption {
    None,
    WEP,
    WPA,
    WPA2,
    WPA3,
}

#[allow(async_fn_in_trait)]
pub trait WifiController {
    type Error;

    /// Start access point mode with given SSID
    async fn start_ap(&mut self, ssid: &str, password: &str) -> Result<(), Self::Error>;

    /// Stop access point mode
    async fn stop_ap(&mut self) -> Result<(), Self::Error>;

    /// Connect to WiFi network with given credentials
    async fn connect_station(
        &mut self,
        ssid: &str,
        password: &str,
        encryption: WifiEncryption,
    ) -> Result<(), Self::Error>;

    /// Disconnect from current WiFi network
    async fn disconnect(&mut self) -> Result<(), Self::Error>;

    /// Check if currently connected to a WiFi network
    fn is_connected(&self) -> bool;
}
