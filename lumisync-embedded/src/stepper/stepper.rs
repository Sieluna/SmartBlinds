use crate::stepper::motor::Motor;

#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    Clockwise,
    CounterClockwise,
}

pub struct Stepper<M: Motor> {
    motor: M,
    current_position: i64,
    target_position: i64,
    speed: f32,
    max_speed: f32,
    acceleration: f32,
    step_interval: u64,
    last_step_time: u64,
    direction: Direction,
    step_count: i64,
    initial_step_delay: f32,
    current_step_delay: f32,
    min_step_delay: f32,
}

impl<M: Motor> Stepper<M> {
    pub fn new(motor: M) -> Self {
        Self {
            motor,
            current_position: 0,
            target_position: 0,
            speed: 0.0,
            max_speed: 1.0,
            acceleration: 1.0,
            step_interval: 0,
            last_step_time: 0,
            direction: Direction::CounterClockwise,
            step_count: 0,
            initial_step_delay: 0.676 * 2.0f32.sqrt() * 1_000_000.0,
            current_step_delay: 0.0,
            min_step_delay: 1_000_000.0,
        }
    }

    pub fn set_current_position(&mut self, position: i64) {
        self.current_position = position;
        self.target_position = position;
        self.speed = 0.0;
        self.step_interval = 0;
        self.step_count = 0;
    }

    pub fn set_speed(&mut self, speed: f32) {
        let speed = speed.clamp(-self.max_speed, self.max_speed);
        if speed != self.speed {
            if speed == 0.0 {
                self.step_interval = 0;
            } else {
                self.step_interval = (1_000_000.0 / speed.abs()) as u64;
                self.direction = if speed > 0.0 {
                    Direction::Clockwise
                } else {
                    Direction::CounterClockwise
                };
            }
            self.speed = speed;
        }
    }

    pub fn set_max_speed(&mut self, speed: f32) {
        let speed = speed.abs();

        if self.max_speed != speed {
            self.max_speed = speed;
            self.min_step_delay = 1_000_000.0 / speed;
            if self.step_count > 0 {
                self.step_count = ((self.speed * self.speed) / (2.0 * self.acceleration)) as i64;
                self.recalculate_speed();
            }
        }
    }

    pub fn set_acceleration(&mut self, acceleration: f32) {
        let acceleration = acceleration.abs();
        if self.acceleration != acceleration {
            if acceleration != 0.0 {
                self.step_count = (self.step_count as f32 * (self.acceleration / acceleration)) as i64;
                self.initial_step_delay = 0.676 * (2.0 / acceleration).sqrt() * 1_000_000.0;
                self.acceleration = acceleration;
                self.recalculate_speed();
            }
        }
    }

    pub fn recalculate_speed(&mut self) -> u64 {
        let distance_to = self.target_position - self.current_position;
        let steps_to_stop = ((self.speed * self.speed) / (2.0 * self.acceleration)).abs() as i64;

        if distance_to == 0 && steps_to_stop <= 1 {
            // Reach the target and it's time to stop
            self.step_interval = 0;
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
            self.current_step_delay = self.current_step_delay
                - ((2.0 * self.current_step_delay)
                / ((4.0 * self.step_count as f32) + 1.0));
            if self.current_step_delay < self.min_step_delay {
                self.current_step_delay = self.min_step_delay;
            }
        }

        self.step_count += 1;
        self.step_interval = self.current_step_delay as u64;
        self.speed = 1_000_000.0 / self.current_step_delay;
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

    pub fn run(&mut self, current_time: u64) -> bool {
        if self.run_speed(current_time) {
            self.recalculate_speed();
        }
        self.speed != 0.0 || self.target_position - self.current_position != 0
    }

    pub fn run_speed(&mut self, current_time: u64) -> bool {
        if self.step_interval == 0 {
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
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

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

        pub fn get_steps(&self) -> Vec<i64> {
            self.steps.borrow().clone()
        }

        pub fn get_status(&self) -> bool {
            self.locked
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
    fn test_stepper_move_to() {
        let mock_motor = MockMotor::new();
        let mut stepper = Stepper::new(mock_motor);

        stepper.set_max_speed(10.0);

        stepper.move_to(5);

        for step in 1..=5 {
            let current_time = step * 10_000;
            stepper.run(current_time);
        }

        let steps = stepper.motor.get_steps();
        assert_eq!(steps.len(), 5);
        assert_eq!(stepper.current_position, 5);
    }

    #[test]
    fn test_stepper_move_relative() {
        let mock_motor = MockMotor::new();
        let mut stepper = Stepper::new(mock_motor);

        stepper.set_current_position(10);
        stepper.set_acceleration(2.0);
        stepper.set_speed(10.0);

        stepper.move_relative(5);

        for step in 1..=5 {
            let current_time = step * 10_000;
            stepper.run(current_time);
        }

        let steps = stepper.motor.get_steps();
        assert_eq!(steps.len(), 5);
        assert_eq!(stepper.current_position, 15);
    }
}