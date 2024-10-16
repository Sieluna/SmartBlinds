use super::PIDParams;

#[derive(Debug)]
pub struct PIDController {
    params: PIDParams,
    previous_error: f32,
    integral: f32,
    last_output: f32,
    last_setpoint: f32,
}

impl PIDController {
    pub fn new(params: PIDParams) -> Self {
        Self {
            params,
            previous_error: 0.0,
            integral: 0.0,
            last_output: 0.0,
            last_setpoint: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.previous_error = 0.0;
        self.integral = 0.0;
        self.last_output = 0.0;
        self.last_setpoint = 0.0;
    }

    pub fn update(&mut self, setpoint: f32, measurement: f32, dt: f32) -> f32 {
        const DT_EPSILON: f32 = 1e-6;
        if dt < DT_EPSILON {
            return self.last_output;
        }

        // Check if setpoint has changed significantly, clear integral term if true
        const SETPOINT_CHANGE_THRESHOLD: f32 = 0.1; // 10% change is considered significant
        let setpoint_change_ratio = if self.last_setpoint != 0.0 {
            (setpoint - self.last_setpoint).abs() / self.last_setpoint.abs()
        } else {
            0.0
        };

        if setpoint_change_ratio > SETPOINT_CHANGE_THRESHOLD {
            self.integral = 0.0;
        }
        self.last_setpoint = setpoint;

        // Calculate error
        let error = setpoint - measurement;

        // Proportional term
        let p_term = self.params.kp * error;

        // Integral term
        self.integral += error * dt;
        let max_integral = self.params.max_output / self.params.ki;
        let min_integral = self.params.min_output / self.params.ki;
        self.integral = self.integral.clamp(min_integral, max_integral);

        let i_term = self.params.ki * self.integral;

        // Derivative term
        let derivative = (error - self.previous_error) / dt;
        let d_term = self.params.kd * derivative;

        // Calculate output
        let mut output = p_term + i_term + d_term;

        // Apply output limits
        output = output.clamp(self.params.min_output, self.params.max_output);

        // Update state
        self.previous_error = error;
        self.last_output = output;

        output
    }

    pub fn get_last_output(&self) -> f32 {
        self.last_output
    }

    pub fn set_params(&mut self, params: PIDParams) {
        self.params = params;
    }

    pub fn get_params(&self) -> &PIDParams {
        &self.params
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid_controller_basic() {
        let params = PIDParams {
            kp: 1.0,
            ki: 0.1,
            kd: 0.01,
            min_output: -100.0,
            max_output: 100.0,
        };

        let mut controller = PIDController::new(params);

        // Test basic update
        let output = controller.update(100.0, 50.0, 0.1);
        assert!(output > 0.0); // Output should be positive since error is positive

        // Test output limits
        let output = controller.update(1000.0, 0.0, 0.1);
        assert_eq!(output, 100.0); // Should be limited to maximum value

        // Test reset
        controller.reset();
        assert_eq!(controller.previous_error, 0.0);
        assert_eq!(controller.integral, 0.0);
        assert_eq!(controller.last_output, 0.0);
    }

    #[test]
    fn test_pid_controller_params() {
        let params = PIDParams {
            kp: 1.0,
            ki: 0.1,
            kd: 0.01,
            min_output: -100.0,
            max_output: 100.0,
        };

        let mut controller = PIDController::new(params);

        // Test parameter retrieval
        let current_params = controller.get_params();
        assert_eq!(current_params.kp, 1.0);
        assert_eq!(current_params.ki, 0.1);
        assert_eq!(current_params.kd, 0.01);

        // Test parameter setting
        let new_params = PIDParams {
            kp: 2.0,
            ki: 0.2,
            kd: 0.02,
            min_output: -200.0,
            max_output: 200.0,
        };
        controller.set_params(new_params);

        let updated_params = controller.get_params();
        assert_eq!(updated_params.kp, 2.0);
        assert_eq!(updated_params.ki, 0.2);
        assert_eq!(updated_params.kd, 0.02);
    }

    #[test]
    fn test_pid_controller_last_output() {
        let params = PIDParams {
            kp: 1.0,
            ki: 0.1,
            kd: 0.01,
            min_output: -100.0,
            max_output: 100.0,
        };

        let mut controller = PIDController::new(params);

        // Test last output value
        let output = controller.update(100.0, 50.0, 0.1);
        assert_eq!(controller.get_last_output(), output);

        // Test last output value after reset
        controller.reset();
        assert_eq!(controller.get_last_output(), 0.0);
    }
}
