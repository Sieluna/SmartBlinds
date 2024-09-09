mod pid;

pub use pid::*;

#[derive(Debug, Clone)]
pub struct PIDParams {
    pub kp: f32,
    pub ki: f32,
    pub kd: f32,
    pub min_output: f32,
    pub max_output: f32,
}

impl Default for PIDParams {
    fn default() -> Self {
        Self {
            kp: 1.0,
            ki: 0.1,
            kd: 0.01,
            min_output: -100.0,
            max_output: 100.0,
        }
    }
}
