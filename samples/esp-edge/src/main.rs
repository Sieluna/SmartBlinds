#![no_std]
#![no_main]

use esp_hal::clock::CpuClock;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;

use log::info;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

use esp_backtrace as _;

extern crate alloc;
use alloc::boxed::Box;
use alloc::format;

use embassy_futures::join::join;
use embassy_net::{Runner, Stack, StackResources};

use esp_wifi::{
    wifi::{ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiState},
    EspWifiController,
};
use esp_wifi::ble::controller::BleConnector;

use trouble_host::prelude::*;

use lumisync_embedded::communication::{EdgeCommunicator, EdgeAnalyzer, transport::{ProtocolWrapper, BleCentralTransport, TcpTransport}};

const SSID: &str = match option_env!("WIFI_SSID") {
    Some(ssid) => ssid,
    None => "your_wifi_ssid",
};
const PASSWORD: &str = match option_env!("WIFI_PASSWORD") {
    Some(password) => password,
    None => "your_password",
};

const SERVER_IP: &str = "192.168.1.100"; // Cloud server IP
const SERVER_PORT: u16 = 8080;

// Edge Configuration
const EDGE_ID: u8 = 1;

// BLE Configuration
const CONNECTIONS_MAX: usize = 3;
const L2CAP_CHANNELS_MAX: usize = 6;

// Known device MAC addresses (in production, this would be loaded from config)
const KNOWN_DEVICES: &[[u8; 6]] = &[
    [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC], // Device 1
    [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBD], // Device 2
];

// When using nightly compiler, it's better to use https://docs.rs/static_cell/2.1.0/static_cell/macro.make_static.html
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    info!("Embassy initialized!");

    let timer1 = TimerGroup::new(peripherals.TIMG0);
    let init = esp_wifi::init(
        timer1.timer0,
        esp_hal::rng::Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
    )
    .unwrap();

    let wifi = peripherals.WIFI;
    let (wifi_interface, controller) = esp_wifi::wifi::new(&init, wifi).unwrap();

    // Configure network stack
    let config = embassy_net::Config::dhcpv4(Default::default());
    let seed = (esp_hal::rng::Rng::new(peripherals.RNG).random() as u64) << 32 
        | esp_hal::rng::Rng::new(peripherals.RNG).random() as u64;

    let (stack, runner) = embassy_net::new(
        wifi_interface, 
        config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );

    let stack = mk_static!(Stack<'static>, stack);
    let init = mk_static!(EspWifiController<'static>, init);
    let bluetooth = mk_static!(esp_hal::peripherals::BT, peripherals.BT);

    // Start tasks
    spawner.spawn(wifi_connection_task(controller)).ok();
    spawner.spawn(net_task(runner)).ok();
    spawner.spawn(main_edge_task(stack, init, bluetooth)).ok();

    loop {
        info!("Edge device running...");
        Timer::after(Duration::from_secs(10)).await;
    }
}

#[embassy_executor::task]
async fn wifi_connection_task(mut controller: WifiController<'static>) {
    loop {
        match esp_wifi::wifi::wifi_state() {
            WifiState::StaConnected => {
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await;
            }
            _ => {}
        }

        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: PASSWORD.try_into().unwrap(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            info!("Starting WiFi connection...");
            controller.start_async().await.unwrap();
            info!("WiFi started");
        }

        if matches!(controller.is_started(), Ok(true)) 
            && !matches!(esp_wifi::wifi::wifi_state(), WifiState::StaConnected) {
            match controller.connect_async().await {
                Ok(_) => info!("WiFi connected successfully"),
                Err(e) => {
                    log::error!("WiFi connection failed: {:?}", e);
                    Timer::after(Duration::from_millis(5000)).await;
                }
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

#[embassy_executor::task]
async fn main_edge_task(
    stack: &'static Stack<'static>,
    init: &'static esp_wifi::EspWifiController<'static>,
    bluetooth: &'static esp_hal::peripherals::BT,
) {
    // Wait for network connection
    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    info!("Network connected, waiting for IP configuration...");
    loop {
        if stack.is_config_up() {
            break;
        }
        Timer::after(Duration::from_millis(100)).await;
    }

    if let Some(config) = stack.config_v4() {
        info!("Got IP configuration: {:?}", config);
    }

    // Initialize BLE
    let connector = BleConnector::new(init, *bluetooth);
    let controller: ExternalController<_, 20> = ExternalController::new(connector);

    let address = Address::random([0xff, 0x8f, 0x1b, 0x05, 0xe4, 0xff]);
    info!("Edge BLE address: {:?}", address);

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> = 
        HostResources::new();
    let stack_ble = trouble_host::new(controller, &mut resources).set_random_address(address);
    let Host { central, mut runner, .. } = stack_ble.build();

    // Create static buffers for TCP transport
    let tcp_rx_buffer = mk_static!([u8; 1536], [0u8; 1536]);
    let tcp_tx_buffer = mk_static!([u8; 1536], [0u8; 1536]);

    // Create transport layers
    let tcp_transport = TcpTransport::new(
        *stack,
        SERVER_IP,
        SERVER_PORT,
        tcp_rx_buffer,
        tcp_tx_buffer,
    ).await;

    let ble_transport = BleCentralTransport::new();

    // Handle transport creation results
    match tcp_transport {
        Ok(tcp_transport) => {
            // Create protocol wrappers
            let tcp_wrapper = ProtocolWrapper::new(tcp_transport);
            let ble_wrapper = ProtocolWrapper::new(ble_transport);

            let mut edge_communicator = EdgeCommunicator::new(
                tcp_wrapper, 
                ble_wrapper, 
                EDGE_ID
            );
            let mut edge_analyzer = EdgeAnalyzer::new();

            info!("Edge device initialization completed, starting main loop...");

            let _ = join(
                runner.run(),
                edge_main_loop(&mut edge_communicator, &mut edge_analyzer, central)
            ).await;
        }
        Err(e) => {
            log::error!("Failed to create TCP transport: {:?}", e);
            loop {
                Timer::after(Duration::from_secs(5)).await;
            }
        }
    }
}

async fn edge_main_loop<C: Controller>(
    edge_communicator: &mut EdgeCommunicator<
        ProtocolWrapper<TcpTransport>,
        ProtocolWrapper<BleCentralTransport>
    >,
    edge_analyzer: &mut EdgeAnalyzer,
    mut central: Central<'_, C, DefaultPacketPool>,
) {
    let mut last_heartbeat = embassy_time::Instant::now();
    let heartbeat_interval = Duration::from_secs(30);

    let mut device_connection_attempts = 0u32;
    let max_connection_attempts = 3;

    loop {
        // Try to connect to known devices (as BLE central)
        if device_connection_attempts < max_connection_attempts {
            for &device_mac in KNOWN_DEVICES {
                match connect_to_device(&mut central, device_mac).await {
                    Ok(_) => {
                        info!("Successfully connected to device: {:?}", device_mac);
                    }
                    Err(e) => {
                        log::warn!("Failed to connect to device {:?}: {:?}", device_mac, e);
                    }
                }
            }
            device_connection_attempts += 1;
        }

        // Handle messages from cloud server
        if let Err(e) = edge_communicator.handle_cloud_message().await {
            log::warn!("Error occurred while processing cloud message: {:?}", e);
        }

        // Handle messages from devices
        if let Err(e) = edge_communicator.handle_device_message().await {
            log::warn!("Error occurred while processing device message: {:?}", e);
        }

        // Periodically send heartbeat and device status
        if embassy_time::Instant::now() - last_heartbeat >= heartbeat_interval {
            // Check time sync
            if let Err(e) = edge_communicator.check_time_sync().await {
                log::warn!("Time sync check failed: {:?}", e);
            }

            // Simulate device status report for known devices
            for (i, &_device_mac) in KNOWN_DEVICES.iter().enumerate() {
                let device_id = (i + 1) as i32;
                let position = 50;
                let battery = 85;
                
                if let Err(e) = edge_communicator.report_device_status(device_id, position, battery).await {
                    log::warn!("Failed to send device status for device {}: {:?}", device_id, e);
                }
                
                // Update analyzer
                edge_analyzer.update_device_state(device_id, position, battery);
                if let Some(adjustment) = edge_analyzer.analyze_adjustment_needed(device_id) {
                    info!("Analysis suggests adjusting device {} to position: {}", device_id, adjustment);
                }
            }
            
            last_heartbeat = embassy_time::Instant::now();
        }

        Timer::after(Duration::from_millis(100)).await;
    }
}

async fn connect_to_device<C: Controller>(
    central: &mut Central<'_, C, DefaultPacketPool>,
    device_mac: [u8; 6],
) -> Result<(), Box<dyn core::fmt::Debug + Send + Sync>> {
    let target = Address::random(device_mac);
    let config = ConnectConfig {
        connect_params: Default::default(),
        scan_config: ScanConfig {
            filter_accept_list: &[(target.kind, &target.addr)],
            ..Default::default()
        },
    };

    info!("Attempting to connect to device: {:?}", device_mac);
    
    // Set a timeout for connection attempt
    let connection_timeout = Duration::from_secs(10);
    let connection_future = central.connect(&config);
    
    match embassy_time::with_timeout(connection_timeout, connection_future).await {
        Ok(Ok(_conn)) => {
            info!("Connected to device: {:?}", device_mac);
            // Store connection for later use
            // In a real implementation, you'd manage these connections
            Ok(())
        }
        Ok(Err(e)) => {
            log::warn!("BLE connection failed for device {:?}: {:?}", device_mac, e);
            Err(Box::new(format!("BLE connection failed: {:?}", e)))
        }
        Err(_) => {
            log::warn!("BLE connection timeout for device: {:?}", device_mac);
            Err(Box::new("Connection timeout"))
        }
    }
}
