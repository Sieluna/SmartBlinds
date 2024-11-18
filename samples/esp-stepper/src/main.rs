#![no_std]
#![no_main]

extern crate alloc;

use core::cell::RefCell;
use core::net::Ipv4Addr;
use core::str::FromStr;

use alloc::string::{String, ToString};

use embassy_executor::Spawner;
use embassy_net::{
    udp::PacketMetadata, IpListenEndpoint, Ipv4Cidr, Runner, Stack, StackResources, StaticConfigV4,
};
use embassy_time::{Duration, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Level, Output, OutputConfig},
    rng::Rng,
    timer::timg::TimerGroup,
};
use esp_println::println;
use esp_wifi::{
    init,
    wifi::{
        AccessPointConfiguration, AuthMethod, ClientConfiguration, Configuration,
        WifiController as InternalWifiController, WifiDevice, WifiError,
    },
    EspWifiController,
};
use lumisync_embedded::{
    network::{
        DhcpConfig, ProtocolWrapper, RawTransport, TcpTransport, WifiController, WifiEncryption,
        WifiManager, WifiState,
    },
    stepper::{FourPinMotor, StepMode, Stepper},
    storage::MemoryStorage,
};
use serde::{Deserialize, Serialize};
use static_cell::StaticCell;

// Test protocol for stepper commands
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StepperCommand {
    Move(i32),
    SetSpeed(f32),
    SetAcceleration(f32),
    Home,
    Stop,
    Status,
    Ping,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StepperResponse {
    Ok,
    Error(String),
    Status {
        position: i32,
        target: i32,
        speed: f32,
        running: bool,
    },
    Pong,
}

// WiFi controller wrapper for esp-wifi
pub struct EspWifiWrapper {
    controller: InternalWifiController<'static>,
}

impl EspWifiWrapper {
    pub fn new(controller: InternalWifiController<'static>) -> Self {
        Self { controller }
    }
}

impl WifiController for EspWifiWrapper {
    type Error = WifiError;

    async fn start_ap(&mut self, ssid: &str, password: &str) -> Result<(), Self::Error> {
        let _ = self.controller.stop_async().await;

        let ssid_str = heapless::String::try_from(ssid).unwrap();
        let password_str = heapless::String::try_from(password).unwrap();

        let ap_config = AccessPointConfiguration {
            ssid: ssid_str,
            password: password_str,
            auth_method: if password.is_empty() {
                AuthMethod::None
            } else {
                AuthMethod::WPA2Personal
            },
            ..Default::default()
        };

        let config = Configuration::AccessPoint(ap_config);
        self.controller.set_configuration(&config)?;
        self.controller.start_async().await?;

        println!("AP started: {}", ssid);
        Timer::after(Duration::from_millis(500)).await;
        Ok(())
    }

    async fn stop_ap(&mut self) -> Result<(), Self::Error> {
        self.controller.stop_async().await?;
        println!("AP stopped");
        Ok(())
    }

    async fn connect_station(
        &mut self,
        ssid: &str,
        password: &str,
        _encryption: WifiEncryption,
    ) -> Result<(), Self::Error> {
        let ssid_str = heapless::String::try_from(ssid).unwrap();
        let password_str = heapless::String::try_from(password).unwrap();

        let config = Configuration::Client(ClientConfiguration {
            ssid: ssid_str,
            password: password_str,
            ..Default::default()
        });

        self.controller.set_configuration(&config)?;
        self.controller.start_async().await?;
        self.controller.connect_async().await?;

        println!("Connected to WiFi: {}", ssid);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Self::Error> {
        self.controller.disconnect_async().await?;
        println!("Disconnected from WiFi");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        matches!(self.controller.is_connected(), Ok(true))
    }
}

// When you are okay with using a nightly compiler it's better to use https://docs.rs/static_cell/2.1.0/static_cell/macro.make_static.html
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: StaticCell<$t> = StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

const GW_IP_ADDR_ENV: Option<&'static str> = option_env!("GATEWAY_IP");

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 96 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let mut rng = Rng::new(peripherals.RNG);

    let esp_wifi_ctrl = &*mk_static!(
        EspWifiController<'static>,
        init(timg0.timer0, rng.clone(), peripherals.RADIO_CLK).unwrap()
    );

    let (controller, interfaces) = esp_wifi::wifi::new(&esp_wifi_ctrl, peripherals.WIFI).unwrap();
    let device = interfaces.sta;

    cfg_if::cfg_if! {
        if #[cfg(feature = "esp32")] {
            let timg1 = TimerGroup::new(peripherals.TIMG1);
            esp_hal_embassy::init(timg1.timer0);
        } else {
            use esp_hal::timer::systimer::SystemTimer;
            let systimer = SystemTimer::new(peripherals.SYSTIMER);
            esp_hal_embassy::init(systimer.alarm0);
        }
    }

    let gw_ip_addr_str = GW_IP_ADDR_ENV.unwrap_or("192.168.4.1");
    let gw_ip_addr = Ipv4Addr::from_str(gw_ip_addr_str).unwrap();

    let config = embassy_net::Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(gw_ip_addr, 24),
        gateway: Some(gw_ip_addr),
        dns_servers: Default::default(),
    });

    let seed = (rng.random() as u64) << 32 | rng.random() as u64;
    let (stack, runner) = embassy_net::new(
        device,
        config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );
    let stack = &*mk_static!(Stack<'static>, stack);

    println!("Initializing stepper motor on GPIO 32,33,25,26...");
    let motor = FourPinMotor::new(
        [
            Output::new(peripherals.GPIO32, Level::Low, OutputConfig::default()), // IN1
            Output::new(peripherals.GPIO33, Level::Low, OutputConfig::default()), // IN2
            Output::new(peripherals.GPIO25, Level::Low, OutputConfig::default()), // IN3
            Output::new(peripherals.GPIO26, Level::Low, OutputConfig::default()), // IN4
        ],
        [false; 4],
        StepMode::FullStep,
    );

    let mut stepper = Stepper::new(motor);
    stepper.set_max_speed(500.0);
    stepper.set_acceleration(200.0);
    stepper.set_current_position(0);
    println!("Stepper motor initialized: GPIO32→IN1, GPIO33→IN2, GPIO25→IN3, GPIO26→IN4");

    let stepper_ref = mk_static!(
        RefCell<Stepper<FourPinMotor<Output<'static>>>>,
        RefCell::new(stepper)
    );

    let storage = MemoryStorage::new();
    let wifi_controller = EspWifiWrapper::new(controller);

    let dhcp_config = DhcpConfig {
        server_ip: gw_ip_addr,
        subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
        gateway: gw_ip_addr,
        dns_server: gw_ip_addr,
        pool_start: Ipv4Addr::new(192, 168, 4, 10),
        pool_end: Ipv4Addr::new(192, 168, 4, 100),
        lease_time: 3600,
    };

    let tcp_rx_buffer = mk_static!([u8; 1536], [0u8; 1536]);
    let tcp_tx_buffer = mk_static!([u8; 1536], [0u8; 1536]);
    let dhcp_rx_meta = mk_static!([PacketMetadata; 16], [PacketMetadata::EMPTY; 16]);
    let dhcp_rx_buffer = mk_static!([u8; 1536], [0u8; 1536]);
    let dhcp_tx_meta = mk_static!([PacketMetadata; 16], [PacketMetadata::EMPTY; 16]);
    let dhcp_tx_buffer = mk_static!([u8; 1536], [0u8; 1536]);

    let wifi_manager = WifiManager::new(
        storage,
        wifi_controller,
        "SmartBlinds".to_string(),
        "SmartBlinds".to_string(),
    )
    .with_config_timeout(300)
    .with_max_attempts(3)
    .with_config_port(8080)
    .with_tcp_buffers(tcp_rx_buffer, tcp_tx_buffer)
    .with_dhcp_config(dhcp_config)
    .with_network_stack(stack.clone())
    .with_dhcp_buffers(dhcp_rx_meta, dhcp_rx_buffer, dhcp_tx_meta, dhcp_tx_buffer);

    let wifi_manager_ref = mk_static!(
        RefCell<WifiManager<MemoryStorage, EspWifiWrapper>>,
        RefCell::new(wifi_manager)
    );

    spawner.spawn(net_task(runner)).unwrap();
    spawner.spawn(wifi_manager_task(wifi_manager_ref)).unwrap();
    spawner.spawn(stepper_task(stepper_ref)).unwrap();
    spawner
        .spawn(stepper_server_task(stack, stepper_ref))
        .unwrap();

    println!("System ready - AP: SmartBlinds, Config port: 8080, Stepper port: 8082");

    let mut counter = 0u32;
    loop {
        Timer::after(Duration::from_secs(60)).await;
        counter += 1;
        println!(
            "System running #{} - Free heap: {} bytes",
            counter,
            esp_alloc::HEAP.free()
        );
    }
}

#[embassy_executor::task]
async fn wifi_manager_task(
    wifi_manager: &'static RefCell<WifiManager<MemoryStorage, EspWifiWrapper>>,
) {
    let mut error_count = 0u32;

    loop {
        let tick_result = wifi_manager.borrow_mut().tick().await;

        match tick_result {
            Ok(()) => {
                if error_count > 0 {
                    error_count = 0;
                }

                let current_state = {
                    let manager = wifi_manager.borrow();
                    manager.state().clone()
                };

                static mut LAST_STATE: Option<WifiState> = None;
                unsafe {
                    if LAST_STATE.as_ref() != Some(&current_state) {
                        match current_state {
                            WifiState::WaitingForConfig => println!("WiFi: AP mode active"),
                            WifiState::ConnectedSTA => println!("WiFi: Connected to station"),
                            WifiState::ConnectingSTA { attempts } => {
                                println!("WiFi: Connecting (attempt {})", attempts + 1)
                            }
                            WifiState::Failed => println!("WiFi: Connection failed"),
                            _ => {}
                        }
                        LAST_STATE = Some(current_state);
                    }
                }
            }
            Err(e) => {
                error_count += 1;
                if error_count <= 3 {
                    println!("WiFi error: {:?}", e);
                }

                if error_count > 10 {
                    println!("WiFi: Too many errors, restarting...");
                    let _ = wifi_manager.borrow_mut().restart_config().await;
                    error_count = 0;
                }

                Timer::after(Duration::from_secs(2)).await;
            }
        }
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy_executor::task]
async fn stepper_task(stepper: &'static RefCell<Stepper<FourPinMotor<Output<'static>>>>) {
    let mut last_run = embassy_time::Instant::now();

    loop {
        let now = embassy_time::Instant::now();
        if now.duration_since(last_run) >= Duration::from_millis(1) {
            let dt = core::time::Duration::from_millis(now.duration_since(last_run).as_millis());
            stepper.borrow_mut().run(dt);
            last_run = now;
        }
        Timer::after(Duration::from_millis(1)).await;
    }
}

#[embassy_executor::task]
async fn stepper_server_task(
    stack: &'static Stack<'static>,
    stepper: &'static RefCell<Stepper<FourPinMotor<Output<'static>>>>,
) {
    Timer::after(Duration::from_secs(5)).await;

    println!("Starting stepper command server on port 8082...");

    loop {
        match create_tcp_server_transport(stack).await {
            Ok(mut transport) => {
                println!("Stepper server ready on port 8082");

                let mut buffer = [0u8; 512];

                loop {
                    match transport.inner_mut().receive_bytes(&mut buffer).await {
                        Ok(Some(len)) => {
                            if let Ok(command_str) = core::str::from_utf8(&buffer[..len]) {
                                if let Ok(command) =
                                    serde_json::from_str::<StepperCommand>(command_str)
                                {
                                    println!("Stepper command: {:?}", command);
                                    let response =
                                        handle_command(command, &mut stepper.borrow_mut());

                                    if let Ok(response_json) = serde_json::to_string(&response) {
                                        if let Err(_) = transport
                                            .inner_mut()
                                            .send_bytes(response_json.as_bytes())
                                            .await
                                        {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        Ok(None) => {
                            Timer::after(Duration::from_millis(10)).await;
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
            }
            Err(_) => {
                Timer::after(Duration::from_secs(5)).await;
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

fn handle_command(
    command: StepperCommand,
    stepper: &mut Stepper<FourPinMotor<Output<'static>>>,
) -> StepperResponse {
    match command {
        StepperCommand::Move(steps) => {
            println!("Moving {} steps", steps);
            stepper.move_to(stepper.get_current_position() + steps as i64);
            StepperResponse::Ok
        }
        StepperCommand::SetSpeed(speed) => {
            if (1.0..=2000.0).contains(&speed) {
                stepper.set_max_speed(speed);
                StepperResponse::Ok
            } else {
                StepperResponse::Error("Invalid speed".to_string())
            }
        }
        StepperCommand::SetAcceleration(accel) => {
            if (1.0..=1000.0).contains(&accel) {
                stepper.set_acceleration(accel);
                StepperResponse::Ok
            } else {
                StepperResponse::Error("Invalid acceleration".to_string())
            }
        }
        StepperCommand::Home => {
            println!("Homing stepper");
            stepper.move_to(0);
            StepperResponse::Ok
        }
        StepperCommand::Stop => {
            println!("Stopping stepper");
            stepper.move_to(stepper.get_current_position());
            StepperResponse::Ok
        }
        StepperCommand::Status => StepperResponse::Status {
            position: stepper.get_current_position() as i32,
            target: stepper.get_target_position() as i32,
            speed: stepper.get_speed(),
            running: stepper.get_current_position() != stepper.get_target_position(),
        },
        StepperCommand::Ping => StepperResponse::Pong,
    }
}

async fn create_tcp_server_transport(
    stack: &'static Stack<'static>,
) -> Result<ProtocolWrapper<TcpTransport>, lumisync_embedded::Error> {
    let rx_buffer = mk_static!([u8; 1536], [0u8; 1536]);
    let tx_buffer = mk_static!([u8; 1536], [0u8; 1536]);

    let mut tcp_transport = TcpTransport::new(stack.clone(), rx_buffer, tx_buffer);

    tcp_transport
        .accept(IpListenEndpoint {
            addr: None,
            port: 8082,
        })
        .await?;

    Ok(ProtocolWrapper::new(tcp_transport))
}
