use embassy_time::{Duration, Timer};

use crate::stepper::Motor;
use crate::time::TimeSync;
use crate::{Error, Result};

use super::motor_control::MotorController;

// Safety timing constants
const EMERGENCY_STOP_RESPONSE_TIME_MS: u64 = 100;
const MAX_OPERATION_TIME_MS: u64 = 60000;
const MIN_COMMAND_INTERVAL_MS: u64 = 100;
const SAFETY_SHUTDOWN_DELAY_MS: u64 = 500;

// Error thresholds
const MAX_CONSECUTIVE_ERRORS: u32 = 5;

#[derive(Debug, Clone, PartialEq)]
pub enum SystemHealthStatus {
    Healthy,
    Warning,
    Critical,
}

pub struct SafetyManager {
    pub last_safety_check_time: u64,
    pub safety_violation_count: u32,
    pub is_emergency_stop_active: bool,
}

impl SafetyManager {
    pub fn new() -> Self {
        Self {
            last_safety_check_time: 0,
            safety_violation_count: 0,
            is_emergency_stop_active: false,
        }
    }

    /// Performs comprehensive operation timeout and safety checks
    pub async fn check_operation_timeout<M>(
        &mut self,
        motor_controller: &mut MotorController<M>,
        time_sync: &TimeSync,
    ) -> Result<()>
    where
        M: Motor,
    {
        let current_time = time_sync.uptime_ms();

        if self
            .check_operation_deadline(motor_controller, current_time)
            .await?
        {
            return Ok(());
        }

        if self
            .check_consecutive_error_threshold(motor_controller)
            .await?
        {
            return Ok(());
        }

        self.update_safety_check_timestamp(current_time);
        Ok(())
    }

    /// Checks if operation deadline has been exceeded
    async fn check_operation_deadline<M>(
        &mut self,
        motor_controller: &mut MotorController<M>,
        current_time: u64,
    ) -> Result<bool>
    where
        M: Motor,
    {
        if let Some(deadline) = motor_controller.operation_deadline {
            if current_time > deadline {
                log::warn!("Operation timeout detected, performing emergency stop");
                self.trigger_emergency_stop(motor_controller).await?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Checks if consecutive error threshold has been exceeded
    async fn check_consecutive_error_threshold<M>(
        &mut self,
        motor_controller: &mut MotorController<M>,
    ) -> Result<bool>
    where
        M: Motor,
    {
        if motor_controller.consecutive_errors > MAX_CONSECUTIVE_ERRORS {
            log::error!("Excessive consecutive errors detected, triggering safety stop");
            self.trigger_emergency_stop(motor_controller).await?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Updates the timestamp of the last safety check
    fn update_safety_check_timestamp(&mut self, current_time: u64) {
        self.last_safety_check_time = current_time;
    }

    /// Triggers emergency stop with full safety protocols
    pub async fn trigger_emergency_stop<M>(
        &mut self,
        motor_controller: &mut MotorController<M>,
    ) -> Result<()>
    where
        M: Motor,
    {
        if self.is_emergency_stop_active {
            log::debug!("Emergency stop already in progress");
            return Ok(());
        }

        log::warn!("SAFETY: Initiating emergency stop procedure");
        self.is_emergency_stop_active = true;
        self.safety_violation_count += 1;

        motor_controller.emergency_stop().await?;

        Timer::after(Duration::from_millis(EMERGENCY_STOP_RESPONSE_TIME_MS)).await;

        self.is_emergency_stop_active = false;
        log::info!("SAFETY: Emergency stop procedure completed");

        Ok(())
    }

    /// Validates motor operation against safety constraints
    pub fn validate_motor_operation<M>(
        &self,
        motor_controller: &MotorController<M>,
        target_position: u8,
    ) -> Result<()>
    where
        M: Motor,
    {
        self.validate_motor_state(motor_controller)?;
        self.validate_position_bounds(target_position)?;
        self.validate_movement_state(motor_controller)?;
        self.validate_calibration_requirements(motor_controller, target_position)?;

        Ok(())
    }

    /// Validates motor is in safe operational state
    fn validate_motor_state<M>(&self, motor_controller: &MotorController<M>) -> Result<()>
    where
        M: Motor,
    {
        if !motor_controller.is_ready_for_commands() {
            log::warn!("Motor not ready for commands");
            return Err(Error::InvalidState);
        }
        Ok(())
    }

    /// Validates position is within acceptable bounds
    fn validate_position_bounds(&self, target_position: u8) -> Result<()> {
        if target_position > 100 {
            log::warn!("Target position {} exceeds maximum limit", target_position);
            return Err(Error::InvalidCommand);
        }
        Ok(())
    }

    /// Validates motor is not currently moving
    fn validate_movement_state<M>(&self, motor_controller: &MotorController<M>) -> Result<()>
    where
        M: Motor,
    {
        if motor_controller.is_moving {
            log::warn!("Cannot start new movement while motor is active");
            return Err(Error::InvalidState);
        }
        Ok(())
    }

    /// Validates calibration requirements for high positions
    fn validate_calibration_requirements<M>(
        &self,
        motor_controller: &MotorController<M>,
        target_position: u8,
    ) -> Result<()>
    where
        M: Motor,
    {
        const HIGH_POSITION_THRESHOLD: u8 = 95;

        if !motor_controller.is_calibration_completed && target_position > HIGH_POSITION_THRESHOLD {
            log::warn!(
                "High position {} requested without proper calibration",
                target_position
            );
            return Err(Error::InvalidState);
        }
        Ok(())
    }

    /// Performs comprehensive system health assessment
    pub fn assess_system_health<M>(
        &self,
        motor_controller: &MotorController<M>,
        time_sync: &TimeSync,
    ) -> SystemHealthStatus
    where
        M: Motor,
    {
        let mut health_status = SystemHealthStatus::Healthy;

        health_status = self.check_safety_violations(health_status);
        health_status = self.check_motor_state_health(motor_controller, health_status);
        health_status = self.check_error_thresholds(motor_controller, health_status);
        health_status = self.check_safety_check_timeliness(time_sync, health_status);

        health_status
    }

    /// Checks for recent safety violations
    fn check_safety_violations(&self, current_status: SystemHealthStatus) -> SystemHealthStatus {
        if self.safety_violation_count > 0 {
            return SystemHealthStatus::Warning;
        }
        current_status
    }

    /// Checks motor state for health indicators
    fn check_motor_state_health<M>(
        &self,
        motor_controller: &MotorController<M>,
        current_status: SystemHealthStatus,
    ) -> SystemHealthStatus
    where
        M: Motor,
    {
        use super::motor_control::MotorState;

        match motor_controller.motor_state {
            MotorState::Error(_) => SystemHealthStatus::Critical,
            MotorState::EmergencyStop => SystemHealthStatus::Warning,
            _ => current_status,
        }
    }

    /// Checks error count thresholds
    fn check_error_thresholds<M>(
        &self,
        motor_controller: &MotorController<M>,
        current_status: SystemHealthStatus,
    ) -> SystemHealthStatus
    where
        M: Motor,
    {
        if motor_controller.consecutive_errors > MAX_CONSECUTIVE_ERRORS / 2 {
            return SystemHealthStatus::Warning;
        }
        current_status
    }

    /// Checks if safety checks are being performed regularly
    fn check_safety_check_timeliness(
        &self,
        time_sync: &TimeSync,
        current_status: SystemHealthStatus,
    ) -> SystemHealthStatus {
        let current_time = time_sync.uptime_ms();

        if current_time - self.last_safety_check_time > MAX_OPERATION_TIME_MS {
            return SystemHealthStatus::Warning;
        }
        current_status
    }

    /// Validates command timing to prevent command flooding
    pub fn validate_command_timing(
        &self,
        last_command_time: u64,
        time_sync: &TimeSync,
    ) -> Result<()> {
        let current_time = time_sync.uptime_ms();

        if current_time >= last_command_time {
            let time_difference = current_time - last_command_time;
            if time_difference < MIN_COMMAND_INTERVAL_MS {
                log::warn!(
                    "Command rate limiting: interval {}ms too short (minimum {}ms)",
                    time_difference,
                    MIN_COMMAND_INTERVAL_MS
                );
                return Err(Error::InvalidCommand);
            }
        } else {
            // Handle potential time rollback
            log::warn!(
                "Time rollback detected: current={}, last={}",
                current_time,
                last_command_time
            );
        }

        Ok(())
    }

    /// Performs complete emergency shutdown sequence
    pub async fn perform_emergency_shutdown<M>(
        &mut self,
        motor_controller: &mut MotorController<M>,
    ) -> Result<()>
    where
        M: Motor,
    {
        log::error!("SAFETY: Initiating complete emergency shutdown");

        self.is_emergency_stop_active = true;
        motor_controller.emergency_stop().await?;

        Timer::after(Duration::from_millis(SAFETY_SHUTDOWN_DELAY_MS)).await;

        log::error!("SAFETY: Emergency shutdown sequence completed");
        Ok(())
    }

    /// Resets safety state for manual recovery procedures
    pub fn reset_safety_state(&mut self) {
        self.safety_violation_count = 0;
        self.is_emergency_stop_active = false;
        log::info!("SAFETY: Safety state has been reset");
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use crate::message::device::MotorController;
    use crate::stepper::Stepper;

    use super::*;

    #[derive(Clone)]
    struct MockMotor {
        enabled: bool,
    }

    impl MockMotor {
        fn new() -> Self {
            Self { enabled: false }
        }
    }

    impl crate::stepper::Motor for MockMotor {
        fn step(&mut self, _step: i64) {}
        fn enable(&mut self) {
            self.enabled = true;
        }
        fn disable(&mut self) {
            self.enabled = false;
        }
    }

    #[tokio::test]
    async fn test_emergency_stop_trigger() {
        let mut manager = SafetyManager::new();
        let motor = MockMotor::new();
        let stepper = Stepper::new(motor);
        let mut motor_controller = MotorController::new(stepper);

        let result = manager.trigger_emergency_stop(&mut motor_controller).await;
        assert!(result.is_ok());
        assert_eq!(manager.safety_violation_count, 1);
    }

    #[test]
    fn test_motor_operation_validation() {
        let manager = SafetyManager::new();
        let motor = MockMotor::new();
        let stepper = Stepper::new(motor);
        let motor_controller = MotorController::new(stepper);

        // Valid position should pass
        let result = manager.validate_motor_operation(&motor_controller, 50);
        assert!(result.is_ok());

        // Invalid position should fail
        let result = manager.validate_motor_operation(&motor_controller, 150);
        assert!(result.is_err());
    }

    #[test]
    fn test_system_health_check() {
        let manager = SafetyManager::new();
        let motor = MockMotor::new();
        let stepper = Stepper::new(motor);
        let motor_controller = MotorController::new(stepper);

        let mut time_sync = crate::time::TimeSync::new();
        let sync_time = OffsetDateTime::from_unix_timestamp(1609459200).unwrap();
        time_sync.sync(sync_time);

        let health = manager.assess_system_health(&motor_controller, &time_sync);
        assert_eq!(health, SystemHealthStatus::Healthy);
    }

    #[test]
    fn test_command_timing_validation() {
        let manager = SafetyManager::new();
        let mut time_sync = crate::time::TimeSync::new();
        let sync_time = OffsetDateTime::from_unix_timestamp(1609459200).unwrap();
        time_sync.sync(sync_time);

        // Simulate some uptime to ensure we have a meaningful current time
        std::thread::sleep(std::time::Duration::from_millis(300));
        let current_time = time_sync.uptime_ms();

        // Normal test with actual time_sync
        let recent_command_time = current_time - 50;
        let result = manager.validate_command_timing(recent_command_time, &time_sync);
        assert!(result.is_err(), "Commands too close together should fail");

        let old_command_time = current_time - 200;
        let result = manager.validate_command_timing(old_command_time, &time_sync);
        assert!(
            result.is_ok(),
            "Commands with sufficient spacing should pass"
        );
    }
}
