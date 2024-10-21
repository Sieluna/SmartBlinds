mod error;
mod wifi;

use error::{Result, WifiError};
use wifi::*;

use tokio::sync::Mutex;

use tauri::{Manager, State};

#[tauri::command]
async fn scan_wifis(state: State<'_, Mutex<WifiState>>) -> Result<Wifi, String> {
    let mut manager = state.lock().await;
    manager.scan_wifis().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn register_device(
    device: Device,
    router_credentials: Credentials,
    state: State<'_, Mutex<WifiState>>,
) -> Result<(), String> {
    let mut manager = state.lock().await;
    manager
        .register_device(device, &router_credentials)
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(Mutex::new(WifiState::new()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![scan_wifis, register_device,])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
