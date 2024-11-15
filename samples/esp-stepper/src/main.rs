#![no_std]
#![no_main]

extern crate alloc;

use core::{net::Ipv4Addr, str::FromStr};
use embassy_executor::Spawner;
use embassy_net::{Ipv4Cidr, Runner, Stack, StackResources, StaticConfigV4};
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
        AccessPointConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiState,
    },
    EspWifiController,
};
use lumisync_embedded::{
    communication::{
        transport::{ProtocolWrapper, TcpTransport},
        MessageTransport,
    },
    stepper::{FourPinMotor, StepMode, Stepper},
};
use serde::{Deserialize, Serialize};

// Test protocol for validation
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
    Error(heapless::String<32>),
    Status {
        position: i32,
        target: i32,
        speed: f32,
        running: bool,
    },
    Pong,
}

// When you are okay with using a nightly compiler it's better to use https://docs.rs/static_cell/2.1.0/static_cell/macro.make_static.html
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
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

    esp_alloc::heap_allocator!(size: 72 * 1024);

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

    let gw_ip_addr_str = GW_IP_ADDR_ENV.unwrap_or("192.168.2.1");
    let gw_ip_addr = Ipv4Addr::from_str(gw_ip_addr_str).expect("failed to parse gateway ip");

    let config = embassy_net::Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(gw_ip_addr, 24),
        gateway: Some(gw_ip_addr),
        dns_servers: Default::default(),
    });

    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    // Init network stack
    let (stack, runner) = embassy_net::new(
        device,
        config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );

    // Create stepper motor
    let motor = FourPinMotor::new(
        [
            Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default()),
            Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default()),
            Output::new(peripherals.GPIO8, Level::Low, OutputConfig::default()),
            Output::new(peripherals.GPIO9, Level::Low, OutputConfig::default()),
        ],
        [false; 4],
        StepMode::FullStep,
    );

    let mut stepper = Stepper::new(motor);
    stepper.set_max_speed(500.0);
    stepper.set_acceleration(200.0);
    stepper.set_current_position(0);

    let stepper_ref = mk_static!(
        core::cell::RefCell<Stepper<FourPinMotor<Output<'static>>>>,
        core::cell::RefCell::new(stepper)
    );

    spawner.spawn(connection(controller)).ok();
    spawner.spawn(net_task(runner)).ok();
    spawner.spawn(run_dhcp(stack, gw_ip_addr_str)).ok();
    spawner.spawn(stepper_task(stepper_ref)).ok();

    let mut rx_buffer = [0; 1536];
    let mut tx_buffer = [0; 1536];

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
    println!(
        "Connect to the AP `esp-wifi` and point your browser to http://{gw_ip_addr_str}:8080/"
    );
    println!("Testing LumiSync network abstractions...");
    while !stack.is_config_up() {
        Timer::after(Duration::from_millis(100)).await
    }
    stack
        .config_v4()
        .inspect(|c| println!("ipv4 config: {c:?}"));

    let tcp_transport = TcpTransport::new_server(stack, 8080, rx_buffer, tx_buffer).await?;

    
    // Wrap with ProtocolWrapper for testing
    Ok(ProtocolWrapper::new(tcp_transport))
    // Main server loop using TcpTransport directly
    loop {
        println!("Wait for connection...");

        // Use TcpTransport::new_server instead of manual TcpSocket
        match create_server_transport(stack).await {
            Ok(mut transport) => {
                println!("Client connected - testing abstractions");

                // Handle this connection using your abstractions
                loop {
                    match transport.receive_message().await {
                        Ok(Some(message)) => {
                            // Test postcard deserialization
                            if let Ok(command) =
                                postcard::from_bytes::<StepperCommand>(&message.payload.0)
                            {
                                let response = stepper_ref
                                    .borrow_mut()
                                    .with(|stepper| handle_command(command, stepper));

                                // Test postcard serialization
                                if let Ok(response_bytes) =
                                    postcard::to_vec::<StepperResponse, 64>(&response)
                                {
                                    // Test MessageTransport send
                                    let test_msg =
                                        lumisync_api::Message::test_message(response_bytes);
                                    let _ = transport.send_message(&test_msg).await;
                                }
                            }
                        }
                        Ok(None) => {
                            Timer::after(Duration::from_millis(10)).await;
                        }
                        Err(_) => {
                            println!("Client disconnected");
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                println!("Transport error: {:?}", e);
                Timer::after(Duration::from_millis(1000)).await;
            }
        }
    }
}

// Use TcpTransport::new_server directly
async fn create_server_transport(
    stack: Stack<'static>,
) -> Result<ProtocolWrapper<TcpTransport>, lumisync_embedded::Error> {
    // Create static buffers for each connection
    let rx_buffer = mk_static!([u8; 1536], [0u8; 1536]);
    let tx_buffer = mk_static!([u8; 1536], [0u8; 1536]);

    // Use TcpTransport::new_server to listen for connections
    let tcp_transport = TcpTransport::new_server(stack, 8080, rx_buffer, tx_buffer).await?;

    // Wrap with ProtocolWrapper for testing
    Ok(ProtocolWrapper::new(tcp_transport))
}

fn handle_command(
    command: StepperCommand,
    stepper: &mut Stepper<FourPinMotor<Output<'static>>>,
) -> StepperResponse {
    match command {
        StepperCommand::Move(steps) => {
            println!("Move {} steps", steps);
            stepper.move_to(stepper.get_current_position() + steps as i64);
            StepperResponse::Ok
        }
        StepperCommand::SetSpeed(speed) => {
            println!("Set speed: {:.1}", speed);
            if (1.0..=2000.0).contains(&speed) {
                stepper.set_max_speed(speed);
                StepperResponse::Ok
            } else {
                StepperResponse::Error(heapless::String::from("Invalid speed"))
            }
        }
        StepperCommand::SetAcceleration(accel) => {
            println!("Set acceleration: {:.1}", accel);
            if (1.0..=1000.0).contains(&accel) {
                stepper.set_acceleration(accel);
                StepperResponse::Ok
            } else {
                StepperResponse::Error(heapless::String::from("Invalid acceleration"))
            }
        }
        StepperCommand::Home => {
            println!("Home");
            stepper.move_to(0);
            StepperResponse::Ok
        }
        StepperCommand::Stop => {
            println!("Stop");
            stepper.move_to(stepper.get_current_position());
            StepperResponse::Ok
        }
        StepperCommand::Status => StepperResponse::Status {
            position: stepper.get_current_position() as i32,
            target: stepper.get_target_position() as i32,
            speed: stepper.get_speed(),
            running: stepper.get_current_position() != stepper.get_target_position(),
        },
        StepperCommand::Ping => {
            println!("Ping");
            StepperResponse::Pong
        }
    }
}

#[embassy_executor::task]
async fn stepper_task(
    stepper: &'static core::cell::RefCell<Stepper<FourPinMotor<Output<'static>>>>,
) {
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
async fn run_dhcp(stack: Stack<'static>, gw_ip_addr: &'static str) {
    use core::net::{Ipv4Addr, SocketAddrV4};

    use edge_dhcp::{
        io::{self, DEFAULT_SERVER_PORT},
        server::{Server, ServerOptions},
    };
    use edge_nal::UdpBind;
    use edge_nal_embassy::{Udp, UdpBuffers};

    let ip = Ipv4Addr::from_str(gw_ip_addr).expect("dhcp task failed to parse gw ip");

    let mut buf = [0u8; 1500];

    let mut gw_buf = [Ipv4Addr::UNSPECIFIED];

    let buffers = UdpBuffers::<3, 1024, 1024, 10>::new();
    let unbound_socket = Udp::new(stack, &buffers);
    let mut bound_socket = unbound_socket
        .bind(core::net::SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::UNSPECIFIED,
            DEFAULT_SERVER_PORT,
        )))
        .await
        .unwrap();

    loop {
        _ = io::server::run(
            &mut Server::<_, 64>::new_with_et(ip),
            &ServerOptions::new(ip, Some(&mut gw_buf)),
            &mut bound_socket,
            &mut buf,
        )
        .await
        .inspect_err(|e| println!("DHCP server error: {e:?}"));
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    println!("start connection task");
    println!("Device capabilities: {:?}", controller.capabilities());
    loop {
        match esp_wifi::wifi::wifi_state() {
            WifiState::ApStarted => {
                // wait until we're no longer connected
                controller.wait_for_event(WifiEvent::ApStop).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::AccessPoint(AccessPointConfiguration {
                ssid: "esp-wifi".try_into().unwrap(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            println!("Starting wifi");
            controller.start_async().await.unwrap();
            println!("Wifi started!");
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}
