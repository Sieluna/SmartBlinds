#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;

use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_net::{Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_wifi::{
    ble::controller::BleConnector,
    init,
    wifi::{
        AuthMethod, ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent,
        WifiState,
    },
    EspWifiController,
};
use lumisync_embedded::communication::{
    transport::{BleCentralTransport, ProtocolWrapper, TcpTransport},
    EdgeAnalyzer, EdgeCommunicator,
};
use trouble_host::prelude::*;

// WiFi Configuration
const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASSWORD");
const SERVER_IP: &str = "192.168.1.100"; // Cloud server IP
const SERVER_PORT: u16 = 8080;

// Edge Configuration
const EDGE_ID: u8 = 1;

// BLE Configuration
const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 2;

// Known device MAC addresses (in production, this would be loaded from config)
const KNOWN_DEVICES: &[[u8; 6]] = &[
    [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC], // Device 1
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

    esp_alloc::heap_allocator!(size: 128 * 1024);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    let mut rng = esp_hal::rng::Rng::new(peripherals.RNG);

    let timer1 = TimerGroup::new(peripherals.TIMG0);
    let esp_wifi_ctrl = &*mk_static!(
        EspWifiController<'static>,
        init(timer1.timer0, rng.clone(), peripherals.RADIO_CLK,).unwrap()
    );

    let (controller, interfaces) = esp_wifi::wifi::new(esp_wifi_ctrl, peripherals.WIFI).unwrap();
    let wifi_device = interfaces.sta;

    // Configure network stack
    let config = embassy_net::Config::dhcpv4(Default::default());
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    let (stack, runner) = embassy_net::new(
        wifi_device,
        config,
        mk_static!(StackResources<2>, StackResources::<2>::new()),
        seed,
    );

    let stack = mk_static!(Stack<'static>, stack);

    log::info!("Network stack initialized");

    // Start tasks
    spawner.spawn(wifi_connection_task(controller)).ok();
    spawner.spawn(net_task(runner)).ok();
    spawner
        .spawn(main_edge_task(stack, esp_wifi_ctrl, peripherals.BT))
        .ok();

    loop {
        Timer::after(Duration::from_secs(30)).await;
        log::info!(
            "Edge device running, WiFi: {:?}",
            esp_wifi::wifi::wifi_state()
        );
    }
}

#[embassy_executor::task]
async fn wifi_connection_task(mut controller: WifiController<'static>) {
    log::info!("WiFi task started, SSID: {}", SSID);

    let mut retry_count = 0u32;
    const MAX_RETRIES: u32 = 5;

    loop {
        match esp_wifi::wifi::wifi_state() {
            WifiState::StaConnected => {
                retry_count = 0;
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                log::info!("WiFi disconnected, will retry");
                Timer::after(Duration::from_millis(3000)).await;
            }
            _ => {}
        }

        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: PASSWORD.try_into().unwrap(),
                auth_method: AuthMethod::WPA2Personal,
                ..Default::default()
            });

            if let Err(e) = controller.set_configuration(&client_config) {
                log::error!("WiFi config failed: {:?}", e);
                Timer::after(Duration::from_millis(5000)).await;
                continue;
            }

            match controller.start_async().await {
                Ok(_) => {
                    log::info!("WiFi controller started");
                }
                Err(e) => {
                    log::error!("WiFi start failed: {:?}", e);
                    Timer::after(Duration::from_millis(5000)).await;
                    continue;
                }
            }
        }

        if matches!(controller.is_started(), Ok(true))
            && !matches!(esp_wifi::wifi::wifi_state(), WifiState::StaConnected)
        {
            retry_count += 1;
            log::info!("WiFi connecting... (attempt {})", retry_count);

            match controller.connect_async().await {
                Ok(_) => {
                    log::info!("WiFi connected!");
                    retry_count = 0;
                    Timer::after(Duration::from_millis(2000)).await;
                }
                Err(e) => {
                    log::error!("WiFi connection failed: {:?}", e);

                    if retry_count >= MAX_RETRIES {
                        log::error!("Max retries reached, restarting controller");
                        retry_count = 0;

                        controller.stop_async().await.ok();
                        Timer::after(Duration::from_millis(5000)).await;
                        continue;
                    }

                    let delay = 3000 + (retry_count * 2000);
                    Timer::after(Duration::from_millis(delay as u64)).await;
                }
            }
        } else {
            Timer::after(Duration::from_millis(2000)).await;
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
    esp_wifi_ctrl: &'static EspWifiController<'static>,
    bluetooth: esp_hal::peripherals::BT<'static>,
) {
    // Wait for network connection with timeout
    let mut wait_attempts = 0;
    const MAX_WAIT_ATTEMPTS: u32 = 30;

    loop {
        if stack.is_link_up() {
            log::info!("Network link ready");
            break;
        }

        wait_attempts += 1;
        if wait_attempts >= MAX_WAIT_ATTEMPTS {
            log::warn!("Network timeout, continuing...");
            break;
        }

        if wait_attempts % 10 == 0 {
            log::info!("Waiting for network... ({}s)", wait_attempts);
        }

        Timer::after(Duration::from_millis(1000)).await;
    }

    // Wait for IP configuration
    wait_attempts = 0;
    loop {
        if stack.is_config_up() {
            log::info!("IP configuration ready");
            break;
        }

        wait_attempts += 1;
        if wait_attempts >= MAX_WAIT_ATTEMPTS {
            log::warn!("IP config timeout, continuing...");
            break;
        }

        Timer::after(Duration::from_millis(1000)).await;
    }

    if let Some(config) = stack.config_v4() {
        log::info!("IP: {}", config.address.address());
    }

    if stack.is_config_up() {
        // Initialize BLE
        log::info!("Initializing BLE...");
        let connector = BleConnector::new(esp_wifi_ctrl, bluetooth);
        let controller: ExternalController<_, 20> = ExternalController::new(connector);

        let address = Address::random([0xff, 0x8f, 0x1b, 0x05, 0xe4, 0xff]);
        log::info!("Edge BLE address: {:?}", address);

        let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
            HostResources::new();
        let stack_ble = trouble_host::new(controller, &mut resources).set_random_address(address);
        let Host {
            central,
            mut runner,
            ..
        } = stack_ble.build();

        // Create BLE transport
        let ble_transport = BleCentralTransport::new();
        let ble_wrapper = ProtocolWrapper::new(ble_transport);

        let tcp_rx_buffer = mk_static!([u8; 1536], [0u8; 1536]);
        let tcp_tx_buffer = mk_static!([u8; 1536], [0u8; 1536]);

        match TcpTransport::new(*stack, SERVER_IP, SERVER_PORT, tcp_rx_buffer, tcp_tx_buffer).await
        {
            Ok(tcp_transport) => {
                log::info!("TCP transport created successfully");

                // Create protocol wrapper
                let tcp_wrapper = ProtocolWrapper::new(tcp_transport);

                // Create Edge communicator and analyzer
                let mut edge_communicator =
                    EdgeCommunicator::new(tcp_wrapper, ble_wrapper, EDGE_ID);
                let mut edge_analyzer = EdgeAnalyzer::new();

                log::info!("Edge device initialization completed, starting main loop...");

                let _ = join(
                    runner.run(),
                    edge_main_loop(&mut edge_communicator, &mut edge_analyzer, central),
                )
                .await;
            }
            Err(e) => {
                log::warn!("Failed to create TCP transport: {:?}", e);
                log::info!("Running in BLE-only mode - no cloud connectivity");

                let _ = runner.run().await;
            }
        }
    } else {
        log::error!("No network, running minimal mode");
        loop {
            Timer::after(Duration::from_secs(30)).await;
        }
    }
}

async fn edge_main_loop<C: Controller>(
    edge_communicator: &mut EdgeCommunicator<
        ProtocolWrapper<TcpTransport>,
        ProtocolWrapper<BleCentralTransport>,
    >,
    edge_analyzer: &mut EdgeAnalyzer,
    mut central: Central<'_, C, DefaultPacketPool>,
) where
    <C as embedded_io::ErrorType>::Error: 'static,
{
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
                        log::info!("Successfully connected to device: {:?}", device_mac);
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
            log::warn!("Process cloud message error: {:?}", e);
        }

        // Handle messages from devices
        if let Err(e) = edge_communicator.handle_device_message().await {
            log::warn!("Process device message error: {:?}", e);
        }

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

                if let Err(e) = edge_communicator
                    .report_device_status(device_id, position, battery)
                    .await
                {
                    log::warn!(
                        "Failed to send device status for device {}: {:?}",
                        device_id,
                        e
                    );
                }

                // Update analyzer
                edge_analyzer.update_device_state(device_id, position, battery);
                if let Some(adjustment) = edge_analyzer.analyze_adjustment_needed(device_id) {
                    log::info!(
                        "Analysis suggests adjusting device {} to position: {}",
                        device_id,
                        adjustment
                    );
                }
            }

            last_heartbeat = embassy_time::Instant::now();
        }

        Timer::after(Duration::from_millis(500)).await;
    }
}

async fn connect_to_device<C: Controller>(
    central: &mut Central<'_, C, DefaultPacketPool>,
    device_mac: [u8; 6],
) -> Result<(), Box<dyn core::fmt::Debug + 'static>>
where
    <C as embedded_io::ErrorType>::Error: 'static,
{
    let target = Address::random(device_mac);
    let config = ConnectConfig {
        connect_params: Default::default(),
        scan_config: ScanConfig {
            filter_accept_list: &[(target.kind, &target.addr)],
            ..Default::default()
        },
    };

    let connection_timeout = Duration::from_secs(5);
    let connection_future = central.connect(&config);

    match embassy_time::with_timeout(connection_timeout, connection_future).await {
        Ok(Ok(_conn)) => {
            log::info!("Connected to device: {:?}", device_mac);
            // Store connection for later use
            // In a real implementation, you'd manage these connections
            Ok(())
        }
        Ok(Err(e)) => Err(Box::new(e)),
        Err(_) => Err(Box::new("Connection timeout")),
    }
}
