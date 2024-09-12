use crate::stepper::motor::Motor;
use embedded_hal::digital::OutputPin;

pub struct FourPinMotor<Pin>
where
    Pin: OutputPin
{
    pins: [Pin; 4],
    pin_inverted: [bool; 4],
}

impl<Pin> FourPinMotor<Pin>
where
    Pin: OutputPin
{
    pub fn new(pins: [Pin; 4], pin_inverted: [bool; 4]) -> Self {
        Self {
            pins,
            pin_inverted,
        }
    }

    fn set_output_pins(&mut self, mask: u8) {
        for (i, pin) in self.pins.iter_mut().enumerate() {
            let state = (mask & (1 << i)) != 0;
            if state ^ self.pin_inverted[i] {
                pin.set_high().ok();
            } else {
                pin.set_low().ok();
            }
        }
    }
}

impl<Pin> Motor for FourPinMotor<Pin>
where
    Pin: OutputPin
{
    fn step(&mut self, step: i64) {
        match step & 0x3 {
            0 => self.set_output_pins(0b0101),
            1 => self.set_output_pins(0b0110),
            2 => self.set_output_pins(0b1010),
            3 => self.set_output_pins(0b1001),
            _ => {}
        }
    }

    fn enable(&mut self) {
        for (pin, &inverted) in self.pins.iter_mut().zip(self.pin_inverted.iter()) {
            if inverted {
                pin.set_low().ok();
            } else {
                pin.set_high().ok();
            }
        }
    }

    fn disable(&mut self) {
        for (pin, &inverted) in self.pins.iter_mut().zip(self.pin_inverted.iter()) {
            if inverted {
                pin.set_high().ok();
            } else {
                pin.set_low().ok();
            }
        }
    }
}

#[cfg(test)]
mod motor_tests {
    use super::*;
    use embedded_hal::digital::ErrorType;
    use std::cell::RefCell;
    use std::convert::Infallible;
    use std::rc::Rc;

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

    impl ErrorType for MockPin { type Error = Infallible; }

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
    fn test_four_pin_motor_full_step_sequence_clockwise() {
        let mock_pin1 = MockPin::new();
        let mock_pin2 = MockPin::new();
        let mock_pin3 = MockPin::new();
        let mock_pin4 = MockPin::new();

        let pins = [mock_pin1, mock_pin2, mock_pin3, mock_pin4];
        let pin_inverted = [false, false, false, false];

        let mut motor = FourPinMotor::new(pins, pin_inverted);

        let steps = [0, 1, 2, 3, 0, 1, 2, 3];

        for &step_phase in &steps {
            motor.step(step_phase);
        }

        let expected_states = [
            vec![true, false, false, true],
            vec![false, true, false, true],
            vec![false, true, true, false],
            vec![true, false, true, false],
            vec![true, false, false, true],
            vec![false, true, false, true],
            vec![false, true, true, false],
            vec![true, false, true, false],
        ];

        for i in 0..4 {
            assert_eq!(
                motor.pins[i].get_states(),
                expected_states[i],
                "Pin {} states mismatch",
                i + 1
            );
        }
    }
}