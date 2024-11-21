use lumisync_api::{Id, SensorData};

use crate::message::device::MotorController as MessageDeviceMotorController;
use crate::stepper::Motor;

use super::motor_control::MotorState;

// Battery and performance constants
const INITIAL_BATTERY_LEVEL: u8 = 100;
const MAX_BATTERY_LEVEL: u8 = 100;
const BATTERY_DRAIN_MOVING: u8 = 3;
const BATTERY_DRAIN_CALIBRATING: u8 = 2;
const BATTERY_DRAIN_IDLE: u8 = 1;

// Error codes
const ERROR_CODE_OK: u8 = 0;
const ERROR_CODE_GENERAL: u8 = 1;
const ERROR_CODE_EMERGENCY_STOP: u8 = 2;

#[derive(Debug, Default)]
pub struct DeviceStatus {
    pub device_id: Id,
    pub battery_level: u8,
    pub error_code: u8,
    pub last_successful_move: Option<u64>,
    pub consecutive_errors: u32,
    pub emergency_stop_count: u32,
    pub total_moves: u64,
    pub total_errors: u32,
}

pub struct StatusManager {
    pub device_status: DeviceStatus,
}

impl StatusManager {
    pub fn new(device_id: Id) -> Self {
        let mut device_status = DeviceStatus::default();
        device_status.device_id = device_id;
        device_status.battery_level = INITIAL_BATTERY_LEVEL;

        Self { device_status }
    }

    /// Updates battery level with proper bounds checking
    pub fn update_battery_level(&mut self, level: u8) {
        self.device_status.battery_level = level.min(MAX_BATTERY_LEVEL);
    }

    /// Simulates battery drain based on current motor activity
    pub fn simulate_battery_drain<M>(&mut self, motor_controller: &MessageDeviceMotorController<M>)
    where
        M: Motor,
    {
        if self.device_status.battery_level > 0 {
            let drain_amount = self.calculate_battery_drain(&motor_controller.motor_state);
            self.device_status.battery_level = self
                .device_status
                .battery_level
                .saturating_sub(drain_amount);
        }
    }

    /// Calculates battery drain amount based on motor state
    fn calculate_battery_drain(&self, motor_state: &MotorState) -> u8 {
        match motor_state {
            MotorState::Moving => BATTERY_DRAIN_MOVING,
            MotorState::Calibrating => BATTERY_DRAIN_CALIBRATING,
            MotorState::EmergencyStop | MotorState::Error(_) | MotorState::Idle => {
                BATTERY_DRAIN_IDLE
            }
        }
    }

    /// Maps motor state to appropriate error code
    pub fn get_error_code_from_motor_state(&self, motor_state: &MotorState) -> u8 {
        match motor_state {
            MotorState::Idle | MotorState::Moving | MotorState::Calibrating => ERROR_CODE_OK,
            MotorState::EmergencyStop => ERROR_CODE_EMERGENCY_STOP,
            MotorState::Error(_) => ERROR_CODE_GENERAL,
        }
    }

    /// Calculates system resource usage metrics
    pub fn get_system_metrics(&self) -> (f32, f32) {
        const BASE_CPU_USAGE: f32 = 10.0;
        const BASE_MEMORY_USAGE: f32 = 35.0;
        const ERROR_CPU_FACTOR: f32 = 0.5;
        const ERROR_MEMORY_FACTOR: f32 = 0.1;
        const MAX_USAGE: f32 = 95.0;

        let cpu_usage =
            BASE_CPU_USAGE + (self.device_status.total_errors as f32 * ERROR_CPU_FACTOR);
        let memory_usage =
            BASE_MEMORY_USAGE + (self.device_status.total_errors as f32 * ERROR_MEMORY_FACTOR);

        (cpu_usage.min(MAX_USAGE), memory_usage.min(MAX_USAGE))
    }

    /// Generates realistic sensor data based on time
    pub fn generate_sensor_data(&self, relative_timestamp_ms: u64) -> SensorData {
        const BASE_TEMPERATURE: f32 = 22.0;
        const TEMPERATURE_VARIATION_AMPLITUDE: f32 = 2.0;
        const BASE_ILLUMINANCE: f32 = 800.0;
        const BASE_HUMIDITY: f32 = 55.0;
        const HUMIDITY_VARIATION_AMPLITUDE: f32 = 10.0;

        let time_factor = relative_timestamp_ms as f32;

        // Temperature simulation with sinusoidal variation
        let temp_variation = (time_factor / 60000.0).sin() * TEMPERATURE_VARIATION_AMPLITUDE;
        let temperature = BASE_TEMPERATURE + temp_variation;

        // Humidity simulation with cosine variation
        let humidity_variation = (time_factor / 120000.0).cos() * HUMIDITY_VARIATION_AMPLITUDE;
        let humidity = BASE_HUMIDITY + humidity_variation;

        SensorData {
            temperature,
            illuminance: BASE_ILLUMINANCE as i32,
            humidity,
        }
    }

    /// Records an error occurrence and updates statistics
    pub fn increment_error_count(&mut self) {
        self.device_status.consecutive_errors += 1;
        self.device_status.total_errors += 1;
        self.device_status.error_code = ERROR_CODE_GENERAL;
    }

    /// Records a successful motor move operation
    pub fn record_successful_move(&mut self, timestamp: u64) {
        self.device_status.last_successful_move = Some(timestamp);
        self.device_status.total_moves += 1;
        self.device_status.consecutive_errors = 0;
        self.device_status.error_code = ERROR_CODE_OK;
    }

    /// Records an emergency stop event
    pub fn record_emergency_stop(&mut self) {
        self.device_status.emergency_stop_count += 1;
        self.device_status.error_code = ERROR_CODE_EMERGENCY_STOP;
    }

    /// Clears error state and counters
    pub fn reset_error_state(&mut self) {
        self.device_status.consecutive_errors = 0;
        self.device_status.error_code = ERROR_CODE_OK;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stepper::{Motor, Stepper};

    #[derive(Clone)]
    struct MockMotor {
        enabled: bool,
    }

    impl MockMotor {
        fn new() -> Self {
            Self { enabled: false }
        }
    }

    impl Motor for MockMotor {
        fn step(&mut self, _step: i64) {}
        fn enable(&mut self) {
            self.enabled = true;
        }
        fn disable(&mut self) {
            self.enabled = false;
        }
    }

    #[test]
    fn test_battery_management_and_status_tracking() {
        let mut manager = StatusManager::new(42);

        // Test initial state
        assert_eq!(manager.device_status.device_id, 42);
        assert_eq!(manager.device_status.battery_level, INITIAL_BATTERY_LEVEL);
        assert_eq!(manager.device_status.error_code, ERROR_CODE_OK);

        // Test battery level updates
        manager.update_battery_level(75);
        assert_eq!(manager.device_status.battery_level, 75);

        // Test battery level clamping
        manager.update_battery_level(150);
        assert_eq!(manager.device_status.battery_level, MAX_BATTERY_LEVEL);

        // Test battery drain simulation
        let motor = MockMotor::new();
        let stepper = Stepper::new(motor);
        let motor_controller = MessageDeviceMotorController::new(stepper);

        let initial_battery = manager.device_status.battery_level;
        manager.simulate_battery_drain(&motor_controller);
        assert!(manager.device_status.battery_level <= initial_battery);

        // Test successful move recording
        manager.record_successful_move(12345);
        assert_eq!(manager.device_status.last_successful_move, Some(12345));
        assert_eq!(manager.device_status.total_moves, 1);
        assert_eq!(manager.device_status.consecutive_errors, 0);

        // Test error counting
        manager.increment_error_count();
        assert_eq!(manager.device_status.consecutive_errors, 1);
        assert_eq!(manager.device_status.total_errors, 1);
        assert_eq!(manager.device_status.error_code, ERROR_CODE_GENERAL);

        // Test emergency stop recording
        manager.record_emergency_stop();
        assert_eq!(manager.device_status.emergency_stop_count, 1);
        assert_eq!(manager.device_status.error_code, ERROR_CODE_EMERGENCY_STOP);
    }

    #[test]
    fn test_system_metrics() {
        let manager = StatusManager::new(42);

        // Test system metrics calculation
        let (cpu_usage, memory_usage) = manager.get_system_metrics();
        assert!(cpu_usage >= 10.0 && cpu_usage <= 95.0);
        assert!(memory_usage >= 35.0 && memory_usage <= 95.0);

        // Test error code mapping
        let error_code = manager.get_error_code_from_motor_state(&MotorState::Idle);
        assert_eq!(error_code, ERROR_CODE_OK);

        let error_code =
            manager.get_error_code_from_motor_state(&MotorState::Error("Test error".to_string()));
        assert_eq!(error_code, ERROR_CODE_GENERAL);

        let error_code = manager.get_error_code_from_motor_state(&MotorState::EmergencyStop);
        assert_eq!(error_code, ERROR_CODE_EMERGENCY_STOP);

        // Test error state reset
        let mut manager_with_errors = StatusManager::new(42);
        manager_with_errors.increment_error_count();
        manager_with_errors.reset_error_state();
        assert_eq!(manager_with_errors.device_status.consecutive_errors, 0);
        assert_eq!(manager_with_errors.device_status.error_code, ERROR_CODE_OK);
    }
}
