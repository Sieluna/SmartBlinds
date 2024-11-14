#![no_std]
#![no_main]

extern crate alloc;

use core::fmt::Write;

use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_time::{Duration, Timer};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    driver::EndpointError,
    Builder,
};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Level, Output, OutputConfig},
    otg_fs::{
        asynch::{Config, Driver},
        Usb,
    },
    timer::timg::TimerGroup,
};
use heapless::String;
use lumisync_embedded::stepper::{FourPinMotor, StepMode, Stepper};

// Device configuration
const DEVICE_NAME: &str = "StepperController";
const DEVICE_VERSION: &str = "1.0.0";

// Command parsing
#[derive(Debug)]
enum StepperCommand {
    Move(i32),
    SetSpeed(f32),
    SetAcceleration(f32),
    Home,
    Stop,
    Status,
    Help,
    Unknown,
}

struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 96 * 1024);

    let timer0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timer0.timer0);

    // Configure GPIO pins for stepper motor
    let pin1 = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());
    let pin2 = Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default());
    let pin3 = Output::new(peripherals.GPIO8, Level::Low, OutputConfig::default());
    let pin4 = Output::new(peripherals.GPIO9, Level::Low, OutputConfig::default());

    // Create stepper motor instance
    let motor = FourPinMotor::new(
        [pin1, pin2, pin3, pin4],
        [false, false, false, false],
        StepMode::FullStep,
    );

    // Wrap the motor in Stepper
    let mut stepper = Stepper::new(motor);
    stepper.set_max_speed(500.0);
    stepper.set_acceleration(200.0);
    stepper.set_current_position(0);

    // Setup USB
    let usb = Usb::new(peripherals.USB0, peripherals.GPIO20, peripherals.GPIO19);

    spawner.spawn(stepper_controller_task(usb, stepper)).ok();

    loop {
        log::info!(
            "Stepper Controller - uptime: {}s",
            embassy_time::Instant::now().as_millis() / 1000
        );
        Timer::after(Duration::from_secs(30)).await;
    }
}

#[embassy_executor::task]
async fn stepper_controller_task(
    usb: Usb<'static>,
    stepper: Stepper<FourPinMotor<Output<'static>>>,
) {
    log::info!("Initializing USB Serial Controller...");

    // Create the driver
    let mut ep_out_buffer = [0u8; 1024];
    let config = Config::default();
    let driver = Driver::new(usb, &mut ep_out_buffer, config);

    // Create embassy-usb Config
    let mut config = embassy_usb::Config::new(0x303A, 0x3001);
    config.manufacturer = Some("LumiSync");
    config.product = Some("Stepper Motor Controller");
    config.serial_number = Some("STEPPER001");

    // Required for windows compatibility
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;

    // Create buffers
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];
    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [],
        &mut control_buf,
    );

    // Create CDC-ACM class
    let mut class = CdcAcmClass::new(&mut builder, &mut state, 64);

    // Build the USB device
    let mut usb = builder.build();

    // Run USB and stepper controller concurrently
    let usb_fut = usb.run();
    let controller_fut = stepper_control_loop(&mut class, stepper);

    join(usb_fut, controller_fut).await;
}

async fn stepper_control_loop<'d>(
    class: &mut CdcAcmClass<'d, Driver<'d>>,
    mut stepper: Stepper<FourPinMotor<Output<'static>>>,
) {
    loop {
        class.wait_connection().await;
        log::info!("USB Serial connected");

        // Send welcome message
        let welcome_msg = format_welcome_message();
        let _ = class.write_packet(welcome_msg.as_bytes()).await;

        let _ = handle_commands(class, &mut stepper).await;
        log::info!("USB Serial disconnected");
    }
}

async fn handle_commands<'d>(
    class: &mut CdcAcmClass<'d, Driver<'d>>,
    stepper: &mut Stepper<FourPinMotor<Output<'static>>>,
) -> Result<(), Disconnected> {
    let mut command_buffer = String::<256>::new();
    let mut buf = [0; 64];
    let mut last_run = embassy_time::Instant::now();

    loop {
        let n = class.read_packet(&mut buf).await?;
        let received = core::str::from_utf8(&buf[0..n]).unwrap_or("");

        for ch in received.chars() {
            if ch == '\n' || ch == '\r' {
                if !command_buffer.is_empty() {
                    let response = process_command(&command_buffer, stepper).await;
                    let _ = class.write_packet(response.as_bytes()).await;
                    command_buffer.clear();
                }
            } else if ch.is_ascii() && command_buffer.len() < command_buffer.capacity() - 1 {
                let _ = command_buffer.push(ch);
            }
        }

        // Run stepper motor tasks
        let now = embassy_time::Instant::now();
        let dt = now.duration_since(last_run);
        let core_dt = core::time::Duration::from_millis(dt.as_millis());
        stepper.run(core_dt);
        last_run = now;

        Timer::after(Duration::from_millis(1)).await;
    }
}

async fn process_command(
    command_str: &str,
    stepper: &mut Stepper<FourPinMotor<Output<'static>>>,
) -> String<512> {
    let command = parse_command(command_str.trim());
    let mut response = String::<512>::new();

    match command {
        StepperCommand::Move(steps) => {
            let _ = write!(response, "Moving {} steps...\r\n", steps);
            stepper.move_to(stepper.get_current_position() + steps as i64);
        }
        StepperCommand::SetSpeed(speed) => {
            stepper.set_max_speed(speed);
            let _ = write!(response, "Max speed set to: {:.2} steps/sec\r\n", speed);
        }
        StepperCommand::SetAcceleration(accel) => {
            stepper.set_acceleration(accel);
            let _ = write!(response, "Acceleration set to: {:.2} steps/sec²\r\n", accel);
        }
        StepperCommand::Home => {
            let _ = write!(response, "Homing to position 0...\r\n");
            stepper.move_to(0);
        }
        StepperCommand::Stop => {
            stepper.move_to(stepper.get_current_position());
            let _ = write!(response, "Motor stopped\r\n");
        }
        StepperCommand::Status => {
            let is_running = stepper.get_current_position() != stepper.get_target_position();
            let _ = write!(
                response,
                "Position: {}, Target: {}, Speed: {:.2}, Running: {}\r\n",
                stepper.get_current_position(),
                stepper.get_target_position(),
                stepper.get_speed(),
                is_running
            );
        }
        StepperCommand::Help => {
            let _ = write!(response, "{}", format_help_message());
        }
        StepperCommand::Unknown => {
            let _ = write!(
                response,
                "Unknown command. Type 'help' for available commands.\r\n"
            );
        }
    }

    response
}

fn parse_command(command_str: &str) -> StepperCommand {
    let parts: heapless::Vec<&str, 4> = command_str.split_whitespace().collect();

    if parts.is_empty() {
        return StepperCommand::Unknown;
    }

    match parts[0].to_ascii_lowercase().as_str() {
        "move" | "m" => {
            if parts.len() >= 2 {
                if let Ok(steps) = parts[1].parse::<i32>() {
                    return StepperCommand::Move(steps);
                }
            }
            StepperCommand::Unknown
        }
        "speed" | "s" => {
            if parts.len() >= 2 {
                if let Ok(speed) = parts[1].parse::<f32>() {
                    return StepperCommand::SetSpeed(speed);
                }
            }
            StepperCommand::Unknown
        }
        "accel" | "a" => {
            if parts.len() >= 2 {
                if let Ok(accel) = parts[1].parse::<f32>() {
                    return StepperCommand::SetAcceleration(accel);
                }
            }
            StepperCommand::Unknown
        }
        "home" | "h" => StepperCommand::Home,
        "stop" | "x" => StepperCommand::Stop,
        "status" | "st" => StepperCommand::Status,
        "help" | "?" => StepperCommand::Help,
        _ => StepperCommand::Unknown,
    }
}

fn format_welcome_message() -> String<512> {
    let mut msg = String::<512>::new();
    let _ = write!(msg, "\r\n=== {} v{} ===\r\n", DEVICE_NAME, DEVICE_VERSION);
    let _ = write!(msg, "Stepper Motor Controller Ready\r\n");
    let _ = write!(msg, "Type 'help' for available commands.\r\n\r\n");
    msg
}

fn format_help_message() -> &'static str {
    "\r\nAvailable Commands:\r\n\
     move <steps>     (m) - Move motor by specified steps (positive or negative)\r\n\
     speed <value>    (s) - Set maximum speed (steps/sec)\r\n\
     accel <value>    (a) - Set acceleration (steps/sec²)\r\n\
     home             (h) - Move to position 0\r\n\
     stop             (x) - Stop motor immediately\r\n\
     status           (st) - Show current motor status\r\n\
     help             (?) - Show this help message\r\n\r\n\
     Examples:\r\n\
     move 100         - Move 100 steps forward\r\n\
     move -50         - Move 50 steps backward\r\n\
     speed 300        - Set max speed to 300 steps/sec\r\n\
     accel 150        - Set acceleration to 150 steps/sec²\r\n\r\n"
}
