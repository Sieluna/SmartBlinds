use embassy_time::{Duration, Timer};
use lumisync_api::Id;

use crate::{
    Error, Result,
    stepper::{Motor, Stepper},
    time::TimeSync,
};

// Position and safety constants
const MAX_POSITION: u8 = 100;
const MAX_SAFE_POSITION_UNCALIBRATED: u8 = 95;
const POSITION_TOLERANCE: u8 = 2;

// Timing constants
const MOTOR_OPERATION_TIMEOUT_MS: u64 = 30000;
const CALIBRATION_TIMEOUT_MS: u64 = 60000;
const RETRY_DELAY_MS: u64 = 500;
const SHUTDOWN_DELAY_MS: u64 = 50;

// Operation limits
const MAX_MOTOR_RETRIES: u8 = 3;
const MAX_STUCK_DETECTION_COUNT: u32 = 100;
const CALIBRATION_SPEED_FACTOR: f32 = 0.5;

// Step calculation constants
const STEPS_PER_REVOLUTION: i64 = 200;
const GEAR_RATIO: i64 = 10;
const FULL_RANGE_DEGREES: i64 = 180;
const MAX_STEPS_PER_MOVE: i64 = 20000;

#[derive(Debug, Default, Clone, PartialEq)]
pub enum MotorState {
    #[default]
    Idle,
    Moving,
    Calibrating,
    EmergencyStop,
    Error(String),
}

/// Controls motor operations with safety features and position tracking
pub struct MotorController<M>
where
    M: Motor,
{
    pub stepper: Stepper<M>,
    pub current_position: u8,
    pub target_position: u8,
    pub motor_state: MotorState,
    pub is_moving: bool,
    pub is_calibration_completed: bool,
    pub operation_deadline: Option<u64>,
    pub consecutive_errors: u32,
    pub total_moves: u64,
}

impl<M> MotorController<M>
where
    M: Motor,
{
    /// Creates a new motor controller for the specified device
    pub fn new(stepper: Stepper<M>) -> Self {
        Self {
            stepper,
            current_position: 0,
            target_position: 0,
            motor_state: MotorState::default(),
            is_moving: false,
            is_calibration_completed: false,
            operation_deadline: None,
            consecutive_errors: 0,
            total_moves: 0,
        }
    }

    /// Sets window position with comprehensive safety validation
    pub async fn set_window_position_safe(
        &mut self,
        position: u8,
        time_sync: &TimeSync,
    ) -> Result<()> {
        let target_position = self.validate_and_clamp_position(position)?;

        if self.is_already_at_target(target_position) {
            log::debug!("Already at target position {}", target_position);
            return Ok(());
        }

        self.validate_operation_preconditions()?;
        self.prepare_for_movement(target_position, time_sync);

        log::info!(
            "Moving window from {} to {}",
            self.current_position,
            target_position
        );

        match self.execute_movement_with_retries(target_position).await {
            Ok(_) => {
                self.complete_successful_movement(target_position);
                Ok(())
            }
            Err(e) => self.handle_movement_failure(e),
        }
    }

    /// Validates and clamps position to safe limits
    fn validate_and_clamp_position(&self, position: u8) -> Result<u8> {
        let target_position = position.min(MAX_POSITION);

        if target_position != position {
            log::warn!("Position clamped from {} to {}", position, target_position);
        }

        if !self.is_calibration_completed && target_position > MAX_SAFE_POSITION_UNCALIBRATED {
            log::warn!(
                "Position {} restricted due to uncalibrated state",
                target_position
            );
            return Err(Error::InvalidState);
        }

        Ok(target_position)
    }

    /// Checks if motor is already at target position within tolerance
    fn is_already_at_target(&self, target_position: u8) -> bool {
        let position_diff = (self.current_position as i16 - target_position as i16).abs();
        position_diff <= POSITION_TOLERANCE as i16
    }

    /// Validates that motor can accept movement commands
    fn validate_operation_preconditions(&self) -> Result<()> {
        if matches!(self.motor_state, MotorState::Error(_)) {
            log::warn!("Cannot move motor in error state");
            return Err(Error::InvalidState);
        }

        if self.is_moving {
            log::warn!("Motor is already moving");
            return Err(Error::InvalidState);
        }

        Ok(())
    }

    /// Prepares motor controller for movement operation
    fn prepare_for_movement(&mut self, target_position: u8, time_sync: &TimeSync) {
        self.target_position = target_position;
        self.is_moving = true;
        self.motor_state = MotorState::Moving;
        self.operation_deadline = Some(time_sync.uptime_ms() + MOTOR_OPERATION_TIMEOUT_MS);
    }

    /// Executes movement with automatic retry logic
    async fn execute_movement_with_retries(&mut self, target_position: u8) -> Result<()> {
        for retry_count in 0..MAX_MOTOR_RETRIES {
            match self.execute_single_movement(target_position).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    log::warn!("Movement attempt {} failed: {:?}", retry_count + 1, e);

                    if retry_count < MAX_MOTOR_RETRIES - 1 {
                        Timer::after(Duration::from_millis(RETRY_DELAY_MS)).await;
                    }
                }
            }
        }

        Err(Error::InvalidState)
    }

    /// Executes a single movement operation
    async fn execute_single_movement(&mut self, target_position: u8) -> Result<()> {
        let steps_needed = self.calculate_steps_for_position(target_position);
        log::debug!(
            "Calculated {} steps for position {}",
            steps_needed,
            target_position
        );

        self.stepper.enable_motor();
        self.stepper.move_to(steps_needed);

        self.wait_for_movement_completion().await?;
        self.perform_gradual_shutdown().await;

        Ok(())
    }

    /// Waits for motor movement to complete with stuck detection
    async fn wait_for_movement_completion(&mut self) -> Result<()> {
        let target = self.stepper.get_target_position();
        let mut last_position = self.stepper.get_current_position();
        let mut stuck_count = 0;

        while self.stepper.get_speed() != 0.0 || self.stepper.get_current_position() != target {
            let current_time =
                core::time::Duration::from_micros(embassy_time::Instant::now().as_micros());
            self.stepper.run(current_time);

            if self.detect_motor_stuck(&mut last_position, &mut stuck_count)? {
                break;
            }

            Timer::after(Duration::from_millis(1)).await;
        }

        Ok(())
    }

    /// Detects if motor is stuck and handles the situation
    fn detect_motor_stuck(
        &mut self,
        last_position: &mut i64,
        stuck_count: &mut u32,
    ) -> Result<bool> {
        let current_position = self.stepper.get_current_position();

        if current_position == *last_position {
            *stuck_count += 1;
            if *stuck_count > MAX_STUCK_DETECTION_COUNT {
                log::error!("Motor stuck at position {}", current_position);
                self.stepper.disable_motor();
                return Err(Error::InvalidState);
            }
        } else {
            *stuck_count = 0;
            *last_position = current_position;
        }

        Ok(false)
    }

    /// Completes successful movement and updates state
    fn complete_successful_movement(&mut self, target_position: u8) {
        self.current_position = target_position;
        self.is_moving = false;
        self.motor_state = MotorState::Idle;
        self.consecutive_errors = 0;
        self.total_moves += 1;
        self.operation_deadline = None;

        log::info!("Successfully moved to position {}", target_position);
    }

    /// Handles movement failure and updates error state
    fn handle_movement_failure(&mut self, error: Error) -> Result<()> {
        self.is_moving = false;
        self.motor_state = MotorState::Error("Motor movement failed after retries".to_string());
        self.operation_deadline = None;

        log::error!("Movement failed: {:?}", error);
        Err(error)
    }

    /// Calculates stepper motor steps needed for target position
    fn calculate_steps_for_position(&self, target_position: u8) -> i64 {
        let current = self.current_position as i64;
        let target = target_position.min(MAX_POSITION) as i64;
        let position_diff_percent = target - current;

        if position_diff_percent == 0 {
            return 0;
        }

        let position_diff_degrees = (position_diff_percent * FULL_RANGE_DEGREES) / 100;
        let steps = (position_diff_degrees * STEPS_PER_REVOLUTION * GEAR_RATIO) / 360;

        steps.clamp(-MAX_STEPS_PER_MOVE, MAX_STEPS_PER_MOVE)
    }

    /// Performs emergency stop with immediate motor shutdown
    pub async fn emergency_stop(&mut self) -> Result<()> {
        log::warn!("EMERGENCY STOP activated");

        self.motor_state = MotorState::EmergencyStop;
        self.stepper.disable_motor();
        self.clear_operation_state();

        Timer::after(Duration::from_millis(100)).await;
        self.motor_state = MotorState::Idle;

        log::info!("Emergency stop completed");
        Ok(())
    }

    /// Performs safe motor calibration procedure
    pub async fn calibrate_safe(&mut self, time_sync: &TimeSync) -> Result<()> {
        log::info!("Starting calibration procedure");

        self.prepare_for_calibration(time_sync);
        let original_speed = self.stepper.get_max_speed();
        self.stepper
            .set_max_speed(original_speed * CALIBRATION_SPEED_FACTOR);

        let calibration_result = self.execute_calibration().await;

        self.complete_calibration(original_speed);
        calibration_result
    }

    /// Prepares controller for calibration
    fn prepare_for_calibration(&mut self, time_sync: &TimeSync) {
        self.motor_state = MotorState::Calibrating;
        self.operation_deadline = Some(time_sync.uptime_ms() + CALIBRATION_TIMEOUT_MS);
    }

    /// Executes the calibration movement
    async fn execute_calibration(&mut self) -> Result<()> {
        self.target_position = 0;
        self.is_moving = true;

        match self.execute_single_movement(0).await {
            Ok(_) => {
                self.current_position = 0;
                self.is_calibration_completed = true;
                log::info!("Calibration completed successfully");
                Ok(())
            }
            Err(e) => {
                log::error!("Calibration failed: {:?}", e);
                Err(e)
            }
        }
    }

    /// Completes calibration and restores original settings
    fn complete_calibration(&mut self, original_speed: f32) {
        self.stepper.set_max_speed(original_speed);
        self.is_moving = false;
        self.motor_state = MotorState::Idle;
        self.operation_deadline = None;
    }

    /// Performs gradual motor shutdown with safety monitoring
    async fn perform_gradual_shutdown(&mut self) {
        let mut safety_counter = 0;
        const MAX_SHUTDOWN_ITERATIONS: u32 = 1000;

        while self.stepper.get_speed() != 0.0 && safety_counter < MAX_SHUTDOWN_ITERATIONS {
            let current_time =
                core::time::Duration::from_micros(embassy_time::Instant::now().as_micros());
            self.stepper.run(current_time);
            Timer::after(Duration::from_millis(1)).await;
            safety_counter += 1;
        }

        if safety_counter >= MAX_SHUTDOWN_ITERATIONS {
            log::warn!("Motor failed to stop gracefully, forcing shutdown");
        }

        Timer::after(Duration::from_millis(SHUTDOWN_DELAY_MS)).await;
        self.stepper.disable_motor();
        log::debug!("Motor shutdown completed");
    }

    /// Clears operation state during emergency situations
    fn clear_operation_state(&mut self) {
        self.operation_deadline = None;
        self.is_moving = false;
        self.target_position = self.current_position;
    }

    pub fn is_ready_for_commands(&self) -> bool {
        !matches!(
            self.motor_state,
            MotorState::EmergencyStop | MotorState::Error(_)
        )
    }

    pub fn reset_error_state(&mut self) {
        if matches!(self.motor_state, MotorState::Error(_)) {
            self.motor_state = MotorState::Idle;
            self.consecutive_errors = 0;
            log::info!("Motor error state reset");
        }
    }

    pub fn set_calibration_completed(&mut self, completed: bool) {
        self.is_calibration_completed = completed;
    }
}

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;
    use core::cell::RefCell;

    use crate::stepper::{Motor, Stepper};

    use super::*;

    #[derive(Clone)]
    struct MockMotor {
        position: i64,
        enabled: bool,
        step_count: Rc<RefCell<i32>>,
    }

    impl MockMotor {
        fn new() -> Self {
            Self {
                position: 0,
                enabled: false,
                step_count: Rc::new(RefCell::new(0)),
            }
        }
    }

    impl Motor for MockMotor {
        fn step(&mut self, step: i64) {
            if self.enabled {
                self.position = step;
                *self.step_count.borrow_mut() += 1;
            }
        }

        fn enable(&mut self) {
            self.enabled = true;
        }

        fn disable(&mut self) {
            self.enabled = false;
        }
    }

    #[tokio::test]
    async fn test_step_calculation() {
        let motor = MockMotor::new();
        let stepper = Stepper::new(motor.clone());
        let mut controller = MotorController::new(stepper);

        // Test step calculations
        let test_cases = [
            (0, 25, 250),    // 25% = 250 steps
            (25, 75, 500),   // 50% = 500 steps
            (0, 100, 1000),  // 100% = 1000 steps
            (100, 0, -1000), // -100% = -1000 steps
            (50, 50, 0),     // No change = 0 steps
        ];

        for (start_pos, target_pos, expected_steps) in test_cases.iter() {
            controller.current_position = *start_pos;
            let calculated_steps = controller.calculate_steps_for_position(*target_pos);
            assert_eq!(
                calculated_steps, *expected_steps,
                "Step calculation failed: {}% -> {}%, expected {} steps, got {}",
                start_pos, target_pos, expected_steps, calculated_steps
            );
        }
    }

    #[tokio::test]
    async fn test_motor_error_state_management() {
        let motor = MockMotor::new();
        let stepper = Stepper::new(motor);
        let mut controller = MotorController::new(stepper);

        // Test initial state is ready
        assert!(controller.is_ready_for_commands());
        assert_eq!(controller.motor_state, MotorState::Idle);

        // Test error state prevents commands
        controller.motor_state = MotorState::Error("Test error".to_string());
        assert!(!controller.is_ready_for_commands());

        // Test error reset restores functionality
        controller.reset_error_state();
        assert!(controller.is_ready_for_commands());
        assert_eq!(controller.motor_state, MotorState::Idle);
        assert_eq!(controller.consecutive_errors, 0);
    }

    #[tokio::test]
    async fn test_emergency_stop_clears_operation_state() {
        let motor = MockMotor::new();
        let stepper = Stepper::new(motor);
        let mut controller = MotorController::new(stepper);

        // Set up active operation state
        controller.is_moving = true;
        controller.target_position = 75;
        controller.operation_deadline = Some(12345);

        let result = controller.emergency_stop().await;

        assert!(result.is_ok());
        assert!(!controller.is_moving);
        assert_eq!(controller.operation_deadline, None);
        assert_eq!(controller.target_position, controller.current_position);
    }
}
