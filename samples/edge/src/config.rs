use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use log::info;
use serde::{Deserialize, Serialize};

use crate::error::{Result, SmartBlindsError};

const MAX_STR_LEN: usize = 32;
const TAG_SSID: &str = "wifi_ssid";
const TAG_PASSWORD: &str = "wifi_password";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiConfig {
    pub ssid: String,
    pub password: String,
}

pub struct ConfigManager {
    nvs: EspNvs<NvsDefault>,
}

impl ConfigManager {
    pub fn new(nvs: EspNvs<NvsDefault>) -> Self {
        Self { nvs }
    }

    pub fn get_credentials(&mut self) -> Result<WifiConfig> {
        let mut ssid_buffer: [u8; MAX_STR_LEN] = [0; MAX_STR_LEN];
        let mut password_buffer: [u8; MAX_STR_LEN] = [0; MAX_STR_LEN];

        let ssid = self.nvs.get_str(TAG_SSID, &mut ssid_buffer)?;
        let password = self.nvs.get_str(TAG_PASSWORD, &mut password_buffer)?;

        match (ssid, password) {
            (Some(s), Some(p)) => Ok(WifiConfig {
                ssid: s.to_string(),
                password: p.to_string(),
            }),
            _ => Err(SmartBlindsError::Storage(
                "Credentials not found".to_string(),
            )),
        }
    }

    pub fn save_credentials(&mut self, ssid: &str, password: &str) -> Result<()> {
        self.nvs.set_str(TAG_SSID, ssid)?;
        self.nvs.set_str(TAG_PASSWORD, password)?;
        info!("Successfully saved WiFi credentials");
        Ok(())
    }

    pub fn clear_credentials(&mut self) -> Result<()> {
        self.nvs.remove(TAG_SSID)?;
        self.nvs.remove(TAG_PASSWORD)?;
        info!("Cleared WiFi credentials");
        Ok(())
    }
}
