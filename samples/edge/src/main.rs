mod config;
mod error;
mod wifi;

use std::sync::{Arc, Mutex};

use embedded_svc::http::{Headers, Method};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::prelude::Peripherals;
use esp_idf_svc::http::server::{Configuration, EspHttpServer};
use esp_idf_svc::io::{Read, Write};
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::nvs::*;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use log::info;

use crate::config::ConfigManager;
use crate::error::Result;
use crate::wifi::WifiManager;

const STACK_SIZE: usize = 10240;
const MAX_LEN: usize = 128;

fn setup_http_server(
    config_manager: Arc<Mutex<ConfigManager>>,
    wifi_manager: Arc<Mutex<WifiManager>>,
) -> Result<EspHttpServer<'static>> {
    #[derive(serde::Deserialize)]
    struct WifiConfigForm<'a> {
        ssid: &'a str,
        password: &'a str,
    }

    let mut server = EspHttpServer::new(&Configuration {
        stack_size: STACK_SIZE,
        ..Default::default()
    })
    .unwrap();

    server.fn_handler::<anyhow::Error, _>("/config", Method::Post, move |mut req| {
        let len = req.content_len().unwrap_or(0) as usize;

        if len > MAX_LEN {
            req.into_status_response(413)?
                .write_all("Request too big".as_bytes())?;
            return Ok(());
        }

        let mut buf = vec![0; len];
        req.read_exact(&mut buf)?;
        let mut resp = req.into_ok_response()?;

        if let Ok(form) = serde_json::from_slice::<WifiConfigForm>(&buf) {
            let mut config_manager = config_manager.lock().unwrap();

            let mut wifi_manager = wifi_manager.lock().unwrap();
            wifi_manager.disconnect()?;
            match wifi_manager.connect(form.ssid, form.password) {
                Ok(_) => {
                    let ip_info = wifi_manager.get_ip_info()?;
                    info!("Successfully connected to WiFi with new credentials");
                    info!("WiFi DHCP info: {:?}", ip_info);
                    config_manager.save_credentials(form.ssid, form.password)?;
                }
                Err(e) => {
                    info!("Failed to connect with new credentials: {}", e);
                    wifi_manager.host_ap("SmartBlinds", "12345678")?;
                    info!("Started AP mode");
                }
            }
        } else {
            resp.write_all("JSON error".as_bytes())?;
        }

        Ok(())
    })?;

    Ok(server)
}

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs_partition = EspDefaultNvsPartition::take()?;

    let nvs = EspNvs::new(nvs_partition.clone(), "storage", true)?;
    let wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs_partition))?,
        sys_loop,
    )?;

    let config_manager = Arc::new(Mutex::new(ConfigManager::new(nvs)));
    let wifi_manager = Arc::new(Mutex::new(WifiManager::new(wifi)));

    let _: Result<()> = match config_manager.lock().unwrap().get_credentials() {
        Ok(c) => match wifi_manager.lock().unwrap().connect(&c.ssid, &c.password) {
            Ok(_) => {
                let ip_info = wifi_manager.lock().unwrap().get_ip_info()?;
                info!("WiFi DHCP info: {:?}", ip_info);
                Ok(())
            }
            Err(e) => {
                info!("Failed to connect with stored credentials: {}", e);
                config_manager.lock().unwrap().clear_credentials()?;

                wifi_manager
                    .lock()
                    .unwrap()
                    .host_ap("SmartBlinds", "12345678")?;
                info!("Started AP mode");

                let _ = setup_http_server(config_manager.clone(), wifi_manager.clone());

                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
                Ok(())
            }
        },
        Err(e) => {
            info!("Failed to get credentials: {}", e);
            wifi_manager
                .lock()
                .unwrap()
                .host_ap("SmartBlinds", "12345678")?;
            info!("Started AP mode");

            let _server = setup_http_server(config_manager.clone(), wifi_manager.clone());

            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }

            Ok(())
        }
    };

    Ok(())
}
