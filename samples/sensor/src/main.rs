use esp_idf_svc::log::EspLogger;
use log::info;

fn main() {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    info!("Hello, world!");
}
