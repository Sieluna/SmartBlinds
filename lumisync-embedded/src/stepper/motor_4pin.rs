use embedded_hal::digital::OutputPin;
use crate::stepper::motor::Motor;

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
    pub fn new(pins: [Pin; 4]) -> Self {
        Self {
            pins,
            pin_inverted: [false; 4],
        }
    }

    fn set_output_pins(&mut self, mask: u8) {
        for (i, pin) in self.pins.iter_mut().enumerate() {
            if (mask & (1 << i)) != 0 {
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
            0 => self.set_output_pins(0b1010), // Phase 0: A+ B+
            1 => self.set_output_pins(0b0110), // Phase 1: A- B+
            2 => self.set_output_pins(0b0101), // Phase 2: A- B-
            3 => self.set_output_pins(0b1001), // Phase 3: A+ B-
            _ => {}
        }
    }

    fn enable(&mut self) {
        for pin in self.pins.iter_mut() {
            pin.set_high().ok();
        }
    }

    fn disable(&mut self) {
        for pin in self.pins.iter_mut() {
            pin.set_low().ok();
        }
    }
}