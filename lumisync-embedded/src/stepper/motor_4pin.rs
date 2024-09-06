use embedded_hal::digital::OutputPin;

use super::Motor;

pub struct FourPinMotor<Pin>
where
    Pin: OutputPin,
{
    pins: [Pin; 4],
    pin_inverted: [bool; 4],
}

impl<Pin> FourPinMotor<Pin>
where
    Pin: OutputPin,
{
    pub fn new(pins: [Pin; 4], pin_inverted: [bool; 4]) -> Self {
        Self { pins, pin_inverted }
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
    Pin: OutputPin,
{
    fn step(&mut self, step: i64) {
        match step & 0x3 {
            0 => self.set_output_pins(0b0101), // A1+B1
            1 => self.set_output_pins(0b0011), // A1+A2
            2 => self.set_output_pins(0b0110), // A2+B1
            3 => self.set_output_pins(0b1001), // A1+B2
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
    fn test_step_sequence() {
        let pins = [
            MockPin::new(),
            MockPin::new(),
            MockPin::new(),
            MockPin::new(),
        ];
        let pin_inverted = [false, false, false, false];
        let mut motor = FourPinMotor::new(pins, pin_inverted);

        motor.step(0);
        motor.step(1);
        motor.step(2);
        motor.step(3);

        let expected_states = [
            [true, false, true, false], // 0b0101 - Step 0: A1+B1
            [true, true, false, false], // 0b0011 - Step 1: A1+A2
            [false, true, true, false], // 0b0110 - Step 2: A2+B1
            [true, false, false, true], // 0b1001 - Step 3: A1+B2
        ];

        for (i, pin) in motor.pins.iter().enumerate() {
            let states = pin.get_states();
            assert_eq!(
                states.len(),
                4,
                "Pin {} should have exactly 4 states, got {}",
                i,
                states.len()
            );
            for (j, &state) in states.iter().enumerate() {
                assert_eq!(
                    state, expected_states[j][i],
                    "Pin {} at step {}: expected {}, got {}",
                    i, j, expected_states[j][i], state
                );
            }
        }
    }

    #[test]
    fn test_enable_disable() {
        let pins = [
            MockPin::new(),
            MockPin::new(),
            MockPin::new(),
            MockPin::new(),
        ];
        let pin_inverted = [false, false, false, false];
        let mut motor = FourPinMotor::new(pins, pin_inverted);

        motor.enable();
        for (i, pin) in motor.pins.iter().enumerate() {
            let states = pin.get_states();
            assert_eq!(
                states.len(),
                1,
                "Pin {} should have exactly 1 state after enable, got {}",
                i,
                states.len()
            );
            assert!(states[0], "Pin {} should be high after enable, got low", i);
        }

        motor.disable();
        for (i, pin) in motor.pins.iter().enumerate() {
            let states = pin.get_states();
            assert_eq!(
                states.len(),
                2,
                "Pin {} should have exactly 2 states after disable, got {}",
                i,
                states.len()
            );
            assert!(
                !states[1],
                "Pin {} should be low after disable, got high",
                i
            );
        }
    }

    #[test]
    fn test_inverted_pins() {
        let pins = [
            MockPin::new(),
            MockPin::new(),
            MockPin::new(),
            MockPin::new(),
        ];
        let pin_inverted = [true, true, true, true];
        let mut motor = FourPinMotor::new(pins, pin_inverted);

        motor.enable();
        for (i, pin) in motor.pins.iter().enumerate() {
            let states = pin.get_states();
            assert_eq!(
                states.len(),
                1,
                "Pin {} should have exactly 1 state after enable (inverted), got {}",
                i,
                states.len()
            );
            assert!(
                !states[0],
                "Pin {} should be low after enable (inverted), got high",
                i
            );
        }

        motor.disable();
        for (i, pin) in motor.pins.iter().enumerate() {
            let states = pin.get_states();
            assert_eq!(
                states.len(),
                2,
                "Pin {} should have exactly 2 states after disable (inverted), got {}",
                i,
                states.len()
            );
            assert!(
                states[1],
                "Pin {} should be high after disable (inverted), got low",
                i
            );
        }
    }
}
