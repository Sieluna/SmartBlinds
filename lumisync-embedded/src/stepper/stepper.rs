use core::f32::consts::SQRT_2;
use core::time::Duration;

use super::{Direction, Motor};

pub struct Stepper<M> {
    motor: M,
    current_position: i64,
    target_position: i64,
    speed: f32,
    max_speed: f32,
    acceleration: f32,
    step_interval: Duration,
    last_step_time: Duration,
    direction: Direction,

    step_count: i64,
    initial_step_delay: Duration,
    current_step_delay: Duration,
    min_step_delay: Duration,
}

impl<M> Stepper<M>
where
    M: Motor,
{
    pub fn new(motor: M) -> Self {
        Self {
            motor,
            current_position: 0,
            target_position: 0,
            speed: 0.0,
            max_speed: 1.0,
            acceleration: 1.0,
            step_interval: Duration::ZERO,
            last_step_time: Duration::ZERO,
            direction: Direction::CounterClockwise,
            step_count: 0,
            initial_step_delay: Duration::from_micros((0.676 * SQRT_2 * 1_000_000.0) as u64),
            current_step_delay: Duration::ZERO,
            min_step_delay: Duration::from_micros(1_000_000),
        }
    }

    pub fn set_current_position(&mut self, position: i64) {
        self.current_position = position;
        self.target_position = position;
        self.speed = 0.0;
        self.step_interval = Duration::ZERO;
        self.step_count = 0;
    }

    pub fn set_speed(&mut self, speed: f32) {
        if (speed - self.speed).abs() < f32::EPSILON {
            return;
        }

        let speed = speed.clamp(-self.max_speed, self.max_speed);

        if speed == 0.0 {
            self.step_interval = Duration::ZERO;
        } else {
            self.step_interval = Duration::from_micros((1_000_000.0 / speed.abs()) as u64);
            self.direction = if speed > 0.0 {
                Direction::Clockwise
            } else {
                Direction::CounterClockwise
            };
        }
        self.speed = speed;
    }

    pub fn set_max_speed(&mut self, speed: f32) {
        let speed = speed.abs();

        if self.max_speed != speed {
            self.max_speed = speed;
            self.min_step_delay = Duration::from_micros((1_000_000.0 / speed) as u64);
            if self.step_count > 0 {
                self.step_count = ((self.speed * self.speed) / (2.0 * self.acceleration)) as i64;
                self.recalculate_speed();
            }
        }
    }

    pub fn set_acceleration(&mut self, acceleration: f32) {
        if acceleration == 0.0 {
            return;
        }

        let acceleration = acceleration.abs();
        if self.acceleration != acceleration {
            self.step_count = (self.step_count as f32 * (self.acceleration / acceleration)) as i64;
            self.initial_step_delay = Duration::from_micros(
                (0.676 * libm::sqrtf(2.0 / acceleration) * 1_000_000.0) as u64,
            );
            self.acceleration = acceleration;
            self.recalculate_speed();
        }
    }

    pub fn recalculate_speed(&mut self) -> Duration {
        let distance_to = self.target_position - self.current_position;
        let steps_to_stop = ((self.speed * self.speed) / (2.0 * self.acceleration)).abs() as i64;

        if distance_to == 0 && steps_to_stop <= 1 {
            // Reach the target and it's time to stop
            self.step_interval = Duration::ZERO;
            self.speed = 0.0;
            self.step_count = 0;
            return self.step_interval;
        }

        if distance_to > 0 {
            // We are anticlockwise from the target
            if self.step_count > 0 {
                if steps_to_stop >= distance_to || self.direction == Direction::CounterClockwise {
                    self.step_count = -steps_to_stop; // Start deceleration
                }
            } else if self.step_count < 0 {
                if steps_to_stop < distance_to && self.direction == Direction::Clockwise {
                    self.step_count = -self.step_count; // Start acceleration
                }
            }
        } else if distance_to < 0 {
            // We are clockwise from the target
            if self.step_count > 0 {
                if steps_to_stop >= -distance_to || self.direction == Direction::Clockwise {
                    self.step_count = -steps_to_stop; // Start deceleration
                }
            } else if self.step_count < 0 {
                if steps_to_stop < -distance_to && self.direction == Direction::CounterClockwise {
                    self.step_count = -self.step_count; // Start acceleration
                }
            }
        }

        if self.step_count == 0 {
            self.current_step_delay = self.initial_step_delay;
            self.direction = if distance_to > 0 {
                Direction::Clockwise
            } else {
                Direction::CounterClockwise
            };
        } else {
            let last_step_size = self.current_step_delay.as_secs_f32();
            let new_step_size =
                last_step_size - (last_step_size * 2.0 / ((4.0 * self.step_count as f32) + 1.0));
            self.current_step_delay = Duration::from_secs_f32(new_step_size);
            if self.current_step_delay < self.min_step_delay {
                self.current_step_delay = self.min_step_delay;
            }
        }

        self.step_count += 1;
        self.step_interval = self.current_step_delay;
        self.speed = 1_000_000.0 / self.current_step_delay.as_micros() as f32;
        if self.direction == Direction::CounterClockwise {
            self.speed = -self.speed;
        }

        self.step_interval
    }

    pub fn move_to(&mut self, absolute: i64) {
        if self.target_position != absolute {
            self.target_position = absolute;
            self.recalculate_speed();
        }
    }

    pub fn move_relative(&mut self, relative: i64) {
        self.move_to(self.current_position + relative);
    }

    pub fn run(&mut self, current_time: Duration) -> bool {
        if self.run_speed(current_time) {
            self.recalculate_speed();
        }
        self.speed != 0.0 || self.target_position - self.current_position != 0
    }

    pub fn run_speed(&mut self, current_time: Duration) -> bool {
        if self.step_interval == Duration::ZERO {
            return false;
        }
        if current_time - self.last_step_time >= self.step_interval {
            if self.direction == Direction::Clockwise {
                self.current_position += 1;
            } else {
                self.current_position -= 1;
            }
            self.motor.step(self.current_position);
            self.last_step_time = current_time;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use core::cell::RefCell;

    use alloc::rc::Rc;
    use alloc::vec::Vec;

    use super::*;

    struct MockMotor {
        steps: Rc<RefCell<Vec<i64>>>,
        locked: bool,
    }

    impl MockMotor {
        pub fn new() -> Self {
            Self {
                steps: Rc::new(RefCell::new(Vec::new())),
                locked: false,
            }
        }
    }

    impl Motor for MockMotor {
        fn step(&mut self, position: i64) {
            self.steps.borrow_mut().push(position);
        }

        fn enable(&mut self) {
            self.locked = false;
        }

        fn disable(&mut self) {
            self.locked = true;
        }
    }

    #[test]
    fn test_set_speed() {
        let motor = MockMotor::new();
        let mut stepper = Stepper::new(motor);

        stepper.set_max_speed(1000.0);
        stepper.set_speed(500.0);
        assert_eq!(stepper.step_interval, Duration::from_micros(2000)); // 1e6 / 500 = 2000μs
        assert_eq!(stepper.direction, Direction::Clockwise);

        stepper.set_speed(-300.0);
        assert_eq!(stepper.step_interval, Duration::from_micros(3333)); // 1e6 / 300 ≈ 3333μs
        assert_eq!(stepper.direction, Direction::CounterClockwise);
    }

    #[test]
    fn test_acceleration_calculation() {
        let motor = MockMotor::new();
        let mut stepper = Stepper::new(motor);

        stepper.set_acceleration(1000.0);
        assert_eq!(
            stepper.initial_step_delay,
            Duration::from_micros((0.676 * (2.0f32 / 1000.0).sqrt() * 1_000_000.0) as u64)
        );
    }

    #[test]
    fn test_move_to() {
        let motor = MockMotor::new();
        let mut stepper = Stepper::new(motor);

        stepper.set_current_position(0);
        stepper.move_to(100);
        assert_eq!(stepper.target_position, 100);
        assert_eq!(stepper.current_position, 0);
    }

    #[test]
    fn test_move_relative() {
        let motor = MockMotor::new();
        let mut stepper = Stepper::new(motor);

        stepper.set_current_position(50);
        stepper.move_relative(30);
        assert_eq!(stepper.target_position, 80);
        assert_eq!(stepper.current_position, 50);
    }

    #[test]
    fn test_speed_limits() {
        let motor = MockMotor::new();
        let mut stepper = Stepper::new(motor);

        stepper.set_max_speed(100.0);
        stepper.set_speed(150.0);
        assert_eq!(stepper.speed, 100.0);

        stepper.set_speed(-200.0);
        assert_eq!(stepper.speed, -100.0);
    }

    #[test]
    fn test_run_speed() {
        let motor = MockMotor::new();
        let mut stepper = Stepper::new(motor);

        stepper.set_speed(100.0);
        stepper.set_current_position(0);
        stepper.move_to(10);

        let mut current_time = Duration::ZERO;
        let mut steps_taken = 0;

        while steps_taken < 10 {
            if stepper.run_speed(current_time) {
                steps_taken += 1;
            }
            current_time += Duration::from_micros(100);
        }

        assert_eq!(stepper.current_position, 10);
    }
}
