use embedded_hal::digital::OutputPin;

use super::Motor;

pub struct TwoPinMotor<Pin>
where
    Pin: OutputPin,
{
    step_pin: Pin,
    dir_pin: Pin,
    step_pin_inverted: bool,
    dir_pin_inverted: bool,
    enabled: bool,
}

impl<Pin> TwoPinMotor<Pin>
where
    Pin: OutputPin,
{
    pub fn new(
        step_pin: Pin,
        dir_pin: Pin,
        step_pin_inverted: bool,
        dir_pin_inverted: bool,
    ) -> Self {
        Self {
            step_pin,
            dir_pin,
            step_pin_inverted,
            dir_pin_inverted,
            enabled: false,
        }
    }

    fn set_direction(&mut self, step: i64) {
        let dir_high = if step >= 0 {
            !self.dir_pin_inverted
        } else {
            self.dir_pin_inverted
        };

        if dir_high {
            self.dir_pin.set_high().ok();
        } else {
            self.dir_pin.set_low().ok();
        }
    }

    fn pulse_step(&mut self) {
        if !self.enabled {
            return;
        }

        if self.step_pin_inverted {
            self.step_pin.set_high().ok();
            self.step_pin.set_low().ok();
        } else {
            self.step_pin.set_low().ok();
            self.step_pin.set_high().ok();
        }
    }
}

impl<Pin> Motor for TwoPinMotor<Pin>
where
    Pin: OutputPin,
{
    fn step(&mut self, step: i64) {
        self.set_direction(step);
        self.pulse_step();
    }

    fn enable(&mut self) {
        self.enabled = true;
    }

    fn disable(&mut self) {
        self.enabled = false;
        if self.step_pin_inverted {
            self.step_pin.set_low().ok();
        } else {
            self.step_pin.set_high().ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use core::cell::RefCell;
    use core::convert::Infallible;

    use alloc::rc::Rc;
    use alloc::vec::Vec;

    use embedded_hal::digital::ErrorType;

    use super::*;

    #[derive(Debug, Default)]
    pub struct MockPin {
        pub states: Rc<RefCell<Vec<bool>>>,
    }

    impl MockPin {
        pub fn new() -> Self {
            Self {
                states: Rc::new(RefCell::new(Vec::new())),
            }
        }

        pub fn get_states(&self) -> Vec<bool> {
            self.states.borrow().clone()
        }
    }

    impl ErrorType for MockPin {
        type Error = Infallible;
    }

    impl OutputPin for MockPin {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.states.borrow_mut().push(false);
            Ok(())
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.states.borrow_mut().push(true);
            Ok(())
        }
    }

    #[test]
    fn test_direction_control() {
        let step_pin = MockPin::new();
        let dir_pin = MockPin::new();
        let mut motor = TwoPinMotor::new(step_pin, dir_pin, false, false);

        motor.enable();

        motor.step(1);
        let dir_states = motor.dir_pin.get_states();
        assert_eq!(
            dir_states[0], true,
            "Direction should be high for positive step"
        );

        motor.step(-1);
        let dir_states = motor.dir_pin.get_states();
        assert_eq!(
            dir_states[1], false,
            "Direction should be low for negative step"
        );
    }

    #[test]
    fn test_step_pulse() {
        let step_pin = MockPin::new();
        let dir_pin = MockPin::new();
        let mut motor = TwoPinMotor::new(step_pin, dir_pin, false, false);

        motor.enable();
        motor.step(1);

        let step_states = motor.step_pin.get_states();
        assert_eq!(
            step_states,
            vec![false, true],
            "Step pulse sequence should be low-high"
        );
    }

    #[test]
    fn test_inverted_pins() {
        let step_pin = MockPin::new();
        let dir_pin = MockPin::new();
        let mut motor = TwoPinMotor::new(step_pin, dir_pin, true, true);

        motor.enable();
        motor.step(1);

        let step_states = motor.step_pin.get_states();
        let dir_states = motor.dir_pin.get_states();

        assert_eq!(
            step_states,
            vec![true, false],
            "Inverted step pulse sequence should be high-low"
        );
        assert_eq!(
            dir_states[0], false,
            "Inverted direction should be low for positive step"
        );
    }

    #[test]
    fn test_enable_disable() {
        let step_pin = MockPin::new();
        let dir_pin = MockPin::new();
        let mut motor = TwoPinMotor::new(step_pin, dir_pin, false, false);

        motor.step(1);
        let step_states = motor.step_pin.get_states();
        assert_eq!(
            step_states.len(),
            0,
            "No step pulses should be generated when disabled"
        );

        motor.enable();
        motor.step(1);
        let step_states = motor.step_pin.get_states();
        assert_eq!(
            step_states.len(),
            2,
            "Step pulses should be generated when enabled"
        );

        motor.disable();
        let step_states = motor.step_pin.get_states();
        assert_eq!(
            step_states.last(),
            Some(&true),
            "Step pin should be in inactive state when disabled"
        );
    }
}
