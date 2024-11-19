mod device;
mod error;
mod network;
mod stepper;
mod wifi;

use device::{DeviceAction, DeviceManager};
use network::{NetworkManager, NetworkScanResult};
use stepper::{StepperCommand, StepperCommandResult, StepperController};

use tauri::{Manager, State};
use tokio::sync::Mutex;

pub struct AppState {
    network_manager: Mutex<NetworkManager>,
    device_manager: Mutex<DeviceManager>,
    stepper_controller: StepperController,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            network_manager: Mutex::new(NetworkManager::new()),
            device_manager: Mutex::new(DeviceManager::new()),
            stepper_controller: StepperController::new(),
        }
    }
}

/// Scan for available WiFi networks
#[tauri::command]
async fn scan_networks(state: State<'_, AppState>) -> Result<NetworkScanResult, String> {
    let mut manager = state.network_manager.lock().await;
    manager.scan_networks().await.map_err(|e| e.to_string())
}

/// Manage device operations
#[tauri::command]
async fn manage_device(
    action: DeviceAction,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let mut manager = state.device_manager.lock().await;
    manager
        .execute_action(action)
        .await
        .map_err(|e| e.to_string())
}

/// Execute stepper motor command
#[tauri::command]
async fn execute_stepper_command(
    endpoint: String,
    command: StepperCommand,
    state: State<'_, AppState>,
) -> Result<StepperCommandResult, String> {
    let controller = &state.stepper_controller;
    controller
        .validate_command(&command)
        .map_err(|e| e.to_string())?;
    controller
        .execute_command(&endpoint, command)
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(AppState::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_networks,
            manage_device,
            execute_stepper_command,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
