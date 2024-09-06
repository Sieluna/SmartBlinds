mod motor_2pin;
mod motor_4pin;
mod stepper;

pub use motor_2pin::TwoPinMotor;
pub use motor_4pin::FourPinMotor;
pub use stepper::Stepper;

#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    Clockwise,
    CounterClockwise,
}

pub trait Motor {
    fn step(&mut self, step: i64);

    fn enable(&mut self);

    fn disable(&mut self);
}
