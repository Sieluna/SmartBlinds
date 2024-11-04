#![no_std]
#![no_main]

use log::{info, warn, error};
use esp_backtrace as _;
use esp_alloc as _;

extern crate alloc;
use alloc::format;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

use esp_hal::{
    clock::CpuClock, 
    gpio::{Io, Level, Output}, 
    peripherals::Peripherals,
    timer::systimer::SystemTimer,
    timer::timg::TimerGroup,
};

use esp_wifi::{
    ble::{controller::BleConnector, BleConnector as BleConnectorTrait},
    init,
    EspWifiController,
};

use trouble_host::prelude::*;

use lumisync_embedded::{
    communication::{
        transport::{BlePeripheralTransport, ProtocolWrapper},
        device::DeviceCommunicator,
    },
    stepper::{Motor, TwoPinMotor},
};

// Device configuration
const DEVICE_ID: u32 = 1;
const DEVICE_MAC: [u8; 6] = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
const DEVICE_NAME: &str = "LumiSync-Device-01";

// BLE configuration
const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 2;

// Macro for creating static variables
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // Initialize memory allocator
    esp_alloc::heap_allocator!(size: 72 * 1024);

    // Initialize timer
    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    info!("Device starting up...");

    // Configure GPIO pins for stepper motor
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    let step_pin = Output::new(io.pins.gpio32, Level::Low);
    let dir_pin = Output::new(io.pins.gpio33, Level::Low);
    
    // Create stepper motor instance
    let motor = TwoPinMotor::new(step_pin, dir_pin, false, false);

    // Initialize WiFi/BLE
    let timer1 = TimerGroup::new(peripherals.TIMG0);
    let esp_wifi_ctrl = mk_static!(
        EspWifiController<'static>,
        init(
            timer1.timer0,
            esp_hal::rng::Rng::new(peripherals.RNG),
            peripherals.RADIO_CLK,
        ).unwrap()
    );

    // Start device task
    spawner.spawn(device_task(esp_wifi_ctrl, peripherals.BT, motor)).ok();

    // Main loop
    loop {
        Timer::after(Duration::from_secs(10)).await;
        info!("Device heartbeat");
    }
}

#[embassy_executor::task]
async fn device_task(
    esp_wifi_ctrl: &'static EspWifiController<'static>,
    bluetooth: esp_hal::peripherals::BT,
    motor: TwoPinMotor<Output<'static>, Output<'static>>,
) {
    info!("Starting device communication task...");
    
    loop {
        match run_device_logic(esp_wifi_ctrl, bluetooth, motor.clone()).await {
            Ok(_) => {
                info!("Device logic completed normally");
            }
            Err(e) => {
                error!("Device logic error: {:?}", e);
            }
        }
        
        // Wait before restarting
        Timer::after(Duration::from_secs(5)).await;
        info!("Restarting device logic...");
    }
}

async fn run_device_logic(
    esp_wifi_ctrl: &EspWifiController<'static>,
    bluetooth: esp_hal::peripherals::BT,
    motor: TwoPinMotor<Output<'static>, Output<'static>>,
) -> Result<(), Box<dyn core::error::Error>> {
    // Initialize BLE controller
    let connector = BleConnector::new(esp_wifi_ctrl, bluetooth);
    let controller: ExternalController<_, 20> = ExternalController::new(connector);

    let address = Address::random(DEVICE_MAC);
    info!("Device BLE address: {:?}", address);

    // Create host resources
    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> = 
        HostResources::new();
    
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(address);
    
    let Host { mut peripheral, mut runner, .. } = stack.build();

    // Create device communicator
    let ble_transport = BlePeripheralTransport::new();
    let protocol_wrapper = ProtocolWrapper::new(ble_transport);
    let mut device_communicator = DeviceCommunicator::new(
        protocol_wrapper, 
        motor, 
        DEVICE_MAC, 
        DEVICE_ID
    );

    info!("Device communicator initialized");

    // Run BLE host and device logic
    let ble_task = async {
        loop {
            if let Err(e) = runner.run().await {
                error!("BLE runner error: {:?}", e);
                break;
            }
        }
    };

    let device_task = async {
        let mut ble_transport = BlePeripheralTransport::new();
        
        // Run BLE peripheral service
        let ble_service_task = ble_transport.run(&mut peripheral, DEVICE_NAME);
        
        // Run device communication logic
        let communication_task = device_communication_loop(&mut device_communicator);
        
        // Run both tasks simultaneously
        embassy_futures::select::select(ble_service_task, communication_task).await;
    };

    // Use join to run all tasks concurrently
    embassy_futures::join::join(ble_task, device_task).await;

    Ok(())
}

async fn device_communication_loop<T, M>(
    device_communicator: &mut DeviceCommunicator<T, M>,
) -> Result<(), Box<dyn core::error::Error>> 
where
    T: lumisync_embedded::communication::MessageTransport,
    M: Motor + Clone,
{
    let mut last_status_report = embassy_time::Instant::now();
    let status_report_interval = Duration::from_secs(30); // Report status every 30 seconds
    let mut battery_drain_counter = 0u32;

    info!("Starting device communication loop");

    loop {
        // Handle messages from edge device
        if let Err(e) = device_communicator.handle_edge_message().await {
            warn!("Failed to handle edge message: {:?}", e);
        }

        // Send status report at regular intervals
        let now = embassy_time::Instant::now();
        if now.duration_since(last_status_report) >= status_report_interval {
            // Simulate battery drain
            battery_drain_counter += 1;
            if battery_drain_counter % 10 == 0 {
                device_communicator.simulate_battery_drain();
            }

            let (uptime, is_synced) = device_communicator.get_time_sync_info();
            info!(
                "Device status - Uptime: {}ms, Time synced: {}, Battery: {}%", 
                uptime, 
                is_synced,
                device_communicator.get_device_status().battery_level
            );

            last_status_report = now;
        }

        // Short wait to avoid busy looping
        Timer::after(Duration::from_millis(100)).await;
    }
}
