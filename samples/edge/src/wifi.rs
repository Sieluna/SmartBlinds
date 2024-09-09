use esp_idf_svc::ipv4::IpInfo;
use esp_idf_svc::wifi::*;
use log::info;

use crate::error::{Result, SmartBlindsError};

pub struct WifiManager {
    wifi: BlockingWifi<EspWifi<'static>>,
}

impl WifiManager {
    pub fn new(wifi: BlockingWifi<EspWifi<'static>>) -> Self {
        Self { wifi }
    }

    pub fn connect(&mut self, ssid: &str, password: &str) -> Result<()> {
        let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
            ssid: ssid
                .try_into()
                .map_err(|_| SmartBlindsError::WifiConnection("Invalid SSID".to_string()))?,
            bssid: None,
            auth_method: AuthMethod::WPA2Personal,
            password: password
                .try_into()
                .map_err(|_| SmartBlindsError::WifiConnection("Invalid password".to_string()))?,
            channel: None,
            ..Default::default()
        });

        self.wifi.set_configuration(&wifi_configuration)?;

        self.wifi.start()?;
        info!("WiFi started");

        self.wifi.connect()?;
        info!("WiFi connected");

        self.wifi.wait_netif_up()?;
        info!("WiFi netif up");

        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<()> {
        self.wifi.stop()?;
        info!("WiFi disconnected");
        Ok(())
    }

    pub fn host_ap(&mut self, ssid: &str, password: &str) -> Result<()> {
        let ap_config = Configuration::AccessPoint(AccessPointConfiguration {
            ssid: ssid
                .try_into()
                .map_err(|_| SmartBlindsError::WifiConnection("Invalid SSID".to_string()))?,
            ssid_hidden: false,
            auth_method: AuthMethod::WPA2Personal,
            password: password
                .try_into()
                .map_err(|_| SmartBlindsError::WifiConnection("Invalid password".to_string()))?,
            channel: 1,
            ..Default::default()
        });

        self.wifi.set_configuration(&ap_config)?;

        self.wifi.start()?;
        info!("AP started with SSID: {}", ssid);

        self.wifi.wait_netif_up()?;
        info!("AP netif up");

        Ok(())
    }

    pub fn get_ip_info(&self) -> Result<IpInfo> {
        Ok(self.wifi.wifi().sta_netif().get_ip_info()?)
    }
}
