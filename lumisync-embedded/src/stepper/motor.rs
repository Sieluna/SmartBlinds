pub trait Motor {
    fn step(&mut self, step: i64);

    fn enable(&mut self);

    fn disable(&mut self);
}