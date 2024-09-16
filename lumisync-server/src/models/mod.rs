pub mod event;
pub mod group;
pub mod region;
pub mod region_setting;
pub mod sensor;
pub mod sensor_data;
pub mod setting;
pub mod user;
pub mod user_region;
pub mod window;
pub mod window_setting;

pub trait Table {
    /// The name of the table
    fn name(&self) -> &'static str;

    /// The SQL statement to create the table
    fn create(&self) -> String;

    /// The SQL statement to dispose the table
    fn dispose(&self) -> String;

    /// The dependencies of the table
    fn dependencies(&self) -> Vec<&'static str>;
}
