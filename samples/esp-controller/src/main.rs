#![no_std]
#![no_main]

extern crate alloc;

use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_time::{Duration, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Level, Output, OutputConfig},
    timer::timg::TimerGroup,
};
use esp_wifi::{ble::controller::BleConnector, init, EspWifiController};
use lumisync_embedded::{
    communication::{
        transport::{BlePeripheralTransport, ProtocolWrapper},
        DeviceCommunicator, MessageTransport,
    },
    stepper::{FourPinMotor, Motor, StepMode, Stepper},
};
use trouble_host::prelude::*;

// Device configuration
const DEVICE_ID: i32 = 1;
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

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timer0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timer0.timer0);

    let timer1 = TimerGroup::new(peripherals.TIMG1);
    let esp_wifi_ctrl = mk_static!(
        EspWifiController<'static>,
        init(
            timer1.timer0,
            esp_hal::rng::Rng::new(peripherals.RNG),
            peripherals.RADIO_CLK,
        )
        .unwrap()
    );

    // Configure GPIO pins for stepper motor
    let pin1 = Output::new(peripherals.GPIO25, Level::Low, OutputConfig::default());
    let pin2 = Output::new(peripherals.GPIO26, Level::Low, OutputConfig::default());
    let pin3 = Output::new(peripherals.GPIO32, Level::Low, OutputConfig::default());
    let pin4 = Output::new(peripherals.GPIO33, Level::Low, OutputConfig::default());

    // Create stepper motor instance - 4-pin motor, using full-step mode
    let motor = FourPinMotor::new(
        [pin1, pin2, pin3, pin4],
        [false, false, false, false], // Pins not inverted
        StepMode::FullStep,
    );

    // Start device task
    spawner
        .spawn(device_task(esp_wifi_ctrl, peripherals.BT, motor))
        .ok();

    loop {
        log::info!(
            "Device heartbeat - uptime: {}s",
            embassy_time::Instant::now().as_millis() / 1000
        );
        Timer::after(Duration::from_secs(30)).await;
    }
}

#[embassy_executor::task]
async fn device_task(
    esp_wifi_ctrl: &'static EspWifiController<'static>,
    bluetooth: esp_hal::peripherals::BT<'static>,
    motor: FourPinMotor<Output<'static>>,
) {
    log::info!("Initializing BLE...");

    // Initialize BLE
    let connector = BleConnector::new(esp_wifi_ctrl, bluetooth);
    let controller: ExternalController<_, 20> = ExternalController::new(connector);

    let address = Address::random(DEVICE_MAC);
    log::info!("Device BLE address: {:?}", address);

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> = HostResources::new();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let Host {
        mut peripheral,
        runner,
        ..
    } = stack.build();

    log::info!("BLE initialized, starting device logic...");

    let ble_transport = BlePeripheralTransport::new();
    let protocol_wrapper = ProtocolWrapper::new(ble_transport);

    // Wrap the motor in Stepper
    let mut stepper = Stepper::new(motor);
    stepper.set_max_speed(500.0); // Set max speed
    stepper.set_acceleration(200.0); // Set acceleration
    stepper.set_current_position(0); // Set initial position

    let mut device_communicator =
        DeviceCommunicator::new(protocol_wrapper, stepper, DEVICE_MAC, DEVICE_ID);

    let _ = join(
        ble_task(runner),
        device_communication_loop(&mut peripheral, &mut device_communicator),
    )
    .await;
}

async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(e) = runner.run().await {
            log::info!("[ble_task] error: {:?}", e);
        }
    }
}

async fn device_communication_loop<C: Controller, T, M>(
    peripheral: &mut Peripheral<'_, C, DefaultPacketPool>,
    device_communicator: &mut DeviceCommunicator<T, M>,
) where
    T: MessageTransport,
    M: Motor,
{
    loop {
        log::info!("Starting BLE advertising...");

        match advertise_and_wait_connection(peripheral).await {
            Ok(_) => {
                log::info!("BLE connection established, starting communication...");

                let mut last_heartbeat = embassy_time::Instant::now();
                let heartbeat_interval = Duration::from_secs(10);

                loop {
                    // Handle messages
                    if let Err(e) = device_communicator.handle_edge_message().await {
                        if matches!(e, lumisync_embedded::Error::NotConnected) {
                            log::info!("Connection lost, breaking...");
                            break;
                        }
                        log::info!("Message handling error: {:?}", e);
                    }

                    // Periodic heartbeat
                    let now = embassy_time::Instant::now();
                    if now.duration_since(last_heartbeat) >= heartbeat_interval {
                        device_communicator.simulate_battery_drain();
                        let status = device_communicator.get_device_status();
                        log::info!(
                            "Device status - Battery: {}%, Position: {}",
                            status.battery_level,
                            status.current_position
                        );
                        last_heartbeat = now;
                    }

                    Timer::after(Duration::from_millis(100)).await;
                }
            }
            Err(e) => {
                log::info!("Advertising error: {:?}", e);
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn advertise_and_wait_connection<C: Controller>(
    peripheral: &mut Peripheral<'_, C, DefaultPacketPool>,
) -> Result<(), BleHostError<C::Error>> {
    let mut adv_data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteLocalName(DEVICE_NAME.as_bytes()),
        ],
        &mut adv_data[..],
    )?;

    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &adv_data[..len],
                scan_data: &[],
            },
        )
        .await?;

    log::info!("Advertising started, waiting for connection...");
    let _conn = advertiser.accept().await?;
    log::info!("Connection accepted!");

    Ok(())
}
