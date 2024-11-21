mod message_handler;
mod motor_control;
mod safety_manager;
mod status_manager;

pub use message_handler::MessageHandler;
pub use motor_control::{MotorController, MotorState};
pub use safety_manager::{SafetyManager, SystemHealthStatus};
pub use status_manager::{DeviceStatus, StatusManager};

use alloc::sync::Arc;

use embassy_time::Duration;
use embedded_io_async::{ErrorType, Read, Write};
use lumisync_api::transport::AsyncMessageTransport;
use lumisync_api::{Id, Protocol};

use crate::Result;
use crate::protocol::message::MessageBuilder;
use crate::protocol::uuid_generator::DeviceBasedUuidGenerator;
use crate::stepper::{Motor, Stepper};
use crate::time::TimeSync;

use super::get_device_mac;

pub struct DeviceCommunicator<IO, M>
where
    IO: Read + Write + ErrorType + Unpin,
    IO::Error: core::fmt::Debug,
    M: Motor,
{
    /// BLE transport layer
    transport: AsyncMessageTransport<IO>,
    /// Motor control subsystem
    motor_controller: MotorController<M>,
    /// Message handling subsystem
    message_handler: MessageHandler,
    /// Status management subsystem
    status_manager: StatusManager,
    /// Safety management subsystem
    safety_manager: SafetyManager,
    /// Time synchronization
    time_sync: TimeSync,
    /// Device MAC address for identification
    device_mac: [u8; 6],
}

impl<IO, M> DeviceCommunicator<IO, M>
where
    IO: Read + Write + ErrorType + Unpin,
    IO::Error: core::fmt::Debug,
    M: Motor,
{
    pub fn new(io: IO, stepper: Stepper<M>, device_id: Id) -> Self {
        let transport = AsyncMessageTransport::new(io)
            .with_default_protocol(Protocol::Postcard)
            .with_crc(true);

        let device_mac = get_device_mac(device_id);

        let node_id = lumisync_api::message::NodeId::Device(device_mac);
        let mut time_sync = TimeSync::new();
        time_sync.set_sync_interval(Duration::from_secs(1800));

        let uuid_generator = Arc::new(DeviceBasedUuidGenerator::new(
            device_mac,
            device_id.unsigned_abs() as u32,
        ));

        let message_builder =
            MessageBuilder::new(Protocol::default(), uuid_generator).with_node_id(node_id);

        Self {
            transport,
            motor_controller: MotorController::new(stepper),
            message_handler: MessageHandler::new(message_builder, device_mac),
            status_manager: StatusManager::new(device_id),
            safety_manager: SafetyManager::new(),
            time_sync,
            device_mac,
        }
    }

    /// Main message handling entry point
    pub async fn handle_edge_message(&mut self) -> Result<()> {
        self.message_handler
            .handle_edge_message(
                &mut self.transport,
                &mut self.motor_controller,
                &mut self.status_manager,
                &mut self.safety_manager,
                &self.time_sync,
            )
            .await
    }

    /// Check for operation timeouts
    pub async fn check_operation_timeout(&mut self) -> Result<()> {
        self.safety_manager
            .check_operation_timeout(&mut self.motor_controller, &self.time_sync)
            .await
    }

    /// Get device status
    pub fn get_device_state(&self) -> &DeviceStatus {
        &self.status_manager.device_status
    }

    /// Update battery level
    pub fn update_battery_level(&mut self, level: u8) {
        self.status_manager.device_status.battery_level = level.min(100);
    }

    /// Simulate battery drain
    pub fn simulate_battery_drain(&mut self) {
        if self.status_manager.device_status.battery_level > 0 {
            let drain_amount = match self.motor_controller.motor_state {
                MotorState::Moving => 3,
                MotorState::Calibrating => 2,
                _ => 1,
            };
            self.status_manager.device_status.battery_level = self
                .status_manager
                .device_status
                .battery_level
                .saturating_sub(drain_amount);
        }
    }

    /// Get uptime
    pub fn get_uptime_ms(&self) -> u64 {
        self.time_sync.uptime_ms()
    }

    /// Get time sync info
    pub fn get_time_sync_info(&self) -> (u64, bool) {
        (self.time_sync.uptime_ms(), self.time_sync.is_synced())
    }

    /// Check if ready for commands
    pub fn is_ready_for_commands(&self) -> bool {
        !matches!(
            self.motor_controller.motor_state,
            MotorState::EmergencyStop | MotorState::Error(_)
        )
    }

    /// Get device statistics
    pub fn get_device_statistics(&self) -> (u64, u32, u32, bool) {
        (
            self.status_manager.device_status.total_moves,
            self.status_manager.device_status.total_errors,
            self.status_manager.device_status.emergency_stop_count,
            self.motor_controller.is_calibration_completed,
        )
    }

    /// Reset error state
    pub fn reset_error_state(&mut self) {
        // Reset motor controller error state
        if matches!(self.motor_controller.motor_state, MotorState::Error(_)) {
            self.motor_controller.motor_state = MotorState::Idle;
            self.motor_controller.consecutive_errors = 0;
        }

        // Reset status manager error state
        self.status_manager.device_status.consecutive_errors = 0;
        self.status_manager.device_status.error_code = 0;
    }
}
