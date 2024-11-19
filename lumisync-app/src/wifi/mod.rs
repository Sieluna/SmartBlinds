pub mod platform;

use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub use super::*;

#[derive(Debug, Default, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Ssid(pub String);

#[derive(Debug, Default, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Bssid(pub [u8; 6]);

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Channel(pub u16);

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Band {
    GHz2,
    GHz5,
    GHz6,
    #[default]
    Unknown,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
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
    #[serde(with = "time::serde::iso8601")]
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

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum ConnState {
    Connected,
    #[default]
    Disconnected,
    Authenticating,
    Error(String),
}
