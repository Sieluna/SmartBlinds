use alloc::sync::Arc;

use embassy_time::{Duration, Timer};
use lumisync_api::message::*;
use lumisync_api::{Id, SensorData, SerializationProtocol};
use time::OffsetDateTime;

use crate::message::MessageTransport;
use crate::protocol::message::MessageBuilder;
use crate::protocol::uuid_generator::DeviceBasedUuidGenerator;
use crate::stepper::{Motor, Stepper};
use crate::time::TimeSync;
use crate::{Error, Result};

use super::status::DeviceStatus;

pub struct DeviceCommunicator<T, M>
where
    T: MessageTransport,
    M: Motor,
{
    /// BLE transport layer (for communicating with Edge)
    ble_transport: T,
    /// Stepper controller
    stepper: Stepper<M>,
    /// Message builder
    message_builder: MessageBuilder,
    /// Device status
    device_state: DeviceStatus,
    /// Time synchronization for relative timestamps
    time_sync: TimeSync,
}

impl<T, M> DeviceCommunicator<T, M>
where
    T: MessageTransport,
    M: Motor,
{
    pub fn new(ble_transport: T, stepper: Stepper<M>, device_mac: [u8; 6], device_id: Id) -> Self {
        let node_id = NodeId::Device(device_mac);
        let mut time_sync = TimeSync::new();
        time_sync.set_sync_interval(Duration::from_secs(1800)); // 30 minutes

        // Create UUID generator using device MAC and ID
        let uuid_generator = Arc::new(DeviceBasedUuidGenerator::new(
            device_mac,
            device_id.unsigned_abs() as u32,
        ));

        Self {
            ble_transport,
            stepper,
            message_builder: MessageBuilder::new(SerializationProtocol::default(), uuid_generator)
                .with_node_id(node_id),
            device_state: DeviceStatus {
                device_id,
                ..Default::default()
            },
            time_sync,
        }
    }

    /// Handle incoming messages from Edge device
    pub async fn handle_edge_message(&mut self) -> Result<()> {
        if let Ok(Some(message)) = self.ble_transport.receive_message().await {
            match &message.payload {
                MessagePayload::EdgeCommand(edge_cmd) => {
                    self.process_edge_command(edge_cmd, &message.header.source)
                        .await?;
                }
                _ => {} // Ignore non-command messages
            }
        }
        Ok(())
    }

    /// Process Edge commands
    async fn process_edge_command(&mut self, command: &EdgeCommand, source: &NodeId) -> Result<()> {
        match command {
            EdgeCommand::Actuator {
                actuator_id,
                command,
                ..
            } => {
                // Only process commands for this device
                if *actuator_id == self.device_state.device_id {
                    self.execute_actuator_command(command, source).await?;
                }
            }
            EdgeCommand::RequestHealthStatus { actuator_id } => {
                if *actuator_id == self.device_state.device_id {
                    self.send_health_status(source).await?;
                }
            }
            EdgeCommand::RequestSensorData { actuator_id } => {
                if *actuator_id == self.device_state.device_id {
                    self.send_sensor_data(source).await?;
                }
            }
        }
        Ok(())
    }

    /// Execute actuator commands
    async fn execute_actuator_command(
        &mut self,
        command: &ActuatorCommand,
        source: &NodeId,
    ) -> Result<()> {
        match command {
            ActuatorCommand::SetWindowPosition(position) => {
                self.set_window_position(*position).await?;
                self.send_status_update(source).await?;
            }
            ActuatorCommand::RequestStatus => {
                self.send_status_update(source).await?;
            }
            ActuatorCommand::EmergencyStop => {
                self.emergency_stop().await?;
                self.send_status_update(source).await?;
            }
            ActuatorCommand::Calibrate => {
                self.calibrate().await?;
                self.send_status_update(source).await?;
            }
        }
        Ok(())
    }

    /// Control window position using stepper motor
    /// Position: 0-100, where 0=fully closed, 100=fully open
    async fn set_window_position(&mut self, position: u8) -> Result<()> {
        let position = position.min(100);

        self.device_state.target_position = position;
        self.device_state.is_moving = true;
        self.device_state.error_code = 0;

        let steps_needed = self.calculate_steps_needed(position);
        self.stepper.enable_motor();
        self.stepper.move_to(steps_needed);

        await_stepper_completion(&mut self.stepper).await;
        gradual_shutdown(&mut self.stepper).await;

        self.device_state.current_position = position;
        self.device_state.is_moving = false;

        Ok(())
    }

    /// Calculate stepper motor steps needed for window position
    /// Formula: (position_change% * 180° * 200steps/rev * 10:1_gear) / 360°
    fn calculate_steps_needed(&self, target_position: u8) -> i64 {
        const STEPS_PER_REVOLUTION: i64 = 200; // 1.8° stepper motor
        const GEAR_RATIO: i64 = 10; // 10:1 reduction gearbox
        const FULL_RANGE_DEGREES: i64 = 180; // 180° from closed to open

        let current = self.device_state.current_position as i64;
        let target = target_position as i64;
        let position_diff_percent = target - current;
        let position_diff_degrees = (position_diff_percent * FULL_RANGE_DEGREES) / 100;

        (position_diff_degrees * STEPS_PER_REVOLUTION * GEAR_RATIO) / 360
    }

    /// Emergency stop
    async fn emergency_stop(&mut self) -> Result<()> {
        // Directly disable motor in emergency
        self.stepper.disable_motor();

        // Update device status
        self.device_state.is_moving = false;
        self.device_state.target_position = self.device_state.current_position;

        Ok(())
    }

    /// Calibrate
    async fn calibrate(&mut self) -> Result<()> {
        // Simplified calibration logic: move to position 0
        self.set_window_position(0).await?;
        self.device_state.current_position = 0;
        Ok(())
    }

    /// Send status update
    async fn send_status_update(&mut self, target: &NodeId) -> Result<()> {
        let relative_ts = self.time_sync.uptime_ms();

        let message = self.message_builder.create_device_status(
            target.clone(),
            self.device_state.device_id,
            lumisync_api::WindowData {
                target_position: self.device_state.current_position,
            },
            self.device_state.battery_level,
            self.device_state.error_code,
            relative_ts,
            OffsetDateTime::UNIX_EPOCH, // Edge will convert to UTC
        );

        self.ble_transport
            .send_message(&message)
            .await
            .map_err(|_| Error::NetworkError)?;
        Ok(())
    }

    /// Send health status
    async fn send_health_status(&mut self, target: &NodeId) -> Result<()> {
        let relative_ts = self.time_sync.uptime_ms();

        let message = self.message_builder.create_health_status(
            target.clone(),
            self.device_state.device_id,
            25.0, // cpu_usage
            45.0, // memory_usage
            self.device_state.battery_level,
            -50, // signal_strength (dBm)
            relative_ts,
            OffsetDateTime::UNIX_EPOCH,
        );

        self.ble_transport
            .send_message(&message)
            .await
            .map_err(|_| Error::NetworkError)?;
        Ok(())
    }

    /// Send sensor data
    async fn send_sensor_data(&mut self, target: &NodeId) -> Result<()> {
        let relative_ts = self.time_sync.uptime_ms();

        let message = self.message_builder.create_sensor_data(
            target.clone(),
            self.device_state.device_id,
            SensorData {
                temperature: 23.5,
                illuminance: 1200,
                humidity: 65.0,
            },
            relative_ts,
            OffsetDateTime::UNIX_EPOCH,
        );

        self.ble_transport
            .send_message(&message)
            .await
            .map_err(|_| Error::NetworkError)?;
        Ok(())
    }

    /// Get current device status
    pub fn get_device_status(&self) -> &DeviceStatus {
        &self.device_state
    }

    /// Update battery level with automatic clamping to 0-100 range
    pub fn update_battery_level(&mut self, level: u8) {
        self.device_state.battery_level = level.min(100);
    }

    /// Simulate battery drain
    pub fn simulate_battery_drain(&mut self) {
        if self.device_state.battery_level > 0 {
            self.device_state.battery_level = self.device_state.battery_level.saturating_sub(1);
        }
    }

    /// Get device uptime
    pub fn get_uptime_ms(&self) -> u64 {
        self.time_sync.uptime_ms()
    }

    /// Get time sync status for device monitoring
    pub fn get_time_sync_info(&self) -> (u64, bool) {
        (self.time_sync.uptime_ms(), self.time_sync.is_synced())
    }
}

/// Wait for stepper motor to reach target position
async fn await_stepper_completion<M: Motor>(stepper: &mut Stepper<M>) {
    let mut last_position = stepper.get_current_position();
    let target = stepper.get_target_position();

    while stepper.get_speed() != 0.0 || stepper.get_current_position() != target {
        let current_time =
            core::time::Duration::from_micros(embassy_time::Instant::now().as_micros());
        stepper.run(current_time);

        // Check for motor stuck condition
        if stepper.get_current_position() == last_position {
            // TODO: Implement stuck detection counter and timeout
        }
        last_position = stepper.get_current_position();

        Timer::after(Duration::from_millis(1)).await;
    }
}

/// Implement graceful shutdown
async fn gradual_shutdown<M: Motor>(stepper: &mut Stepper<M>) {
    // First ensure the stepper motor has stopped
    while stepper.get_speed() != 0.0 {
        let current_time =
            core::time::Duration::from_micros(embassy_time::Instant::now().as_micros());
        stepper.run(current_time);
        Timer::after(Duration::from_millis(1)).await;
    }

    // After motor fully stops, apply slight damping to prevent vibration from sudden release
    // In some applications, soft shutdown may need to be implemented via PWM or other methods
    Timer::after(Duration::from_millis(50)).await;

    // Finally safely disable the motor
    stepper.disable_motor();
}

#[cfg(test)]
mod tests {
    use core::cell::RefCell;

    use alloc::rc::Rc;
    use alloc::vec::Vec;

    use lumisync_api::message::*;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use super::*;

    #[derive(Clone)]
    struct MockTransport {
        messages: Vec<Message>,
        receive_queue: Vec<Message>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                messages: Vec::new(),
                receive_queue: Vec::new(),
            }
        }

        fn add_message_to_receive(&mut self, message: Message) {
            self.receive_queue.push(message);
        }

        fn get_sent_messages(&self) -> &Vec<Message> {
            &self.messages
        }
    }

    impl MessageTransport for MockTransport {
        type Error = Error;

        async fn send_message(&mut self, message: &Message) -> Result<()> {
            self.messages.push(message.clone());
            Ok(())
        }

        async fn receive_message(&mut self) -> Result<Option<Message>> {
            Ok(self.receive_queue.pop())
        }
    }

    #[derive(Clone)]
    struct MockMotor {
        position: i64,
        enabled: bool,
        steps: Rc<RefCell<Vec<i64>>>,
    }

    impl MockMotor {
        fn new() -> Self {
            Self {
                position: 0,
                enabled: false,
                steps: Rc::new(RefCell::new(Vec::new())),
            }
        }
    }

    impl Motor for MockMotor {
        fn step(&mut self, step: i64) {
            if self.enabled {
                self.steps.borrow_mut().push(step);
                self.position = step;
            }
        }

        fn enable(&mut self) {
            self.enabled = true;
        }

        fn disable(&mut self) {
            self.enabled = false;
        }
    }

    fn create_edge_command_message(
        source: NodeId,
        target: NodeId,
        actuator_id: lumisync_api::Id,
        command: ActuatorCommand,
    ) -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::UNIX_EPOCH,
                priority: Priority::Regular,
                source,
                target,
            },
            payload: MessagePayload::EdgeCommand(EdgeCommand::Actuator {
                actuator_id,
                sequence: 1,
                command,
            }),
        }
    }

    #[test]
    fn test_stepper_calculation_formula() {
        let transport = MockTransport::new();
        let motor = MockMotor::new();
        let stepper = Stepper::new(motor);
        let device_mac = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let mut communicator = DeviceCommunicator::new(transport, stepper, device_mac, 1);

        // Test formula: (position_change% * 180° * 200steps * 10gear) / 360°
        let test_cases = [
            (0, 100, 1000),  // 100% = (100*180*200*10)/360 = 1000 steps
            (100, 0, -1000), // -100% = (-100*180*200*10)/360 = -1000 steps
            (50, 51, 5),     // 1% = (1*180*200*10)/360 = 5 steps
            (99, 100, 5),    // 1% = 5 steps
            (1, 0, -5),      // -1% = -5 steps
            (0, 0, 0),       // 0% = 0 steps
            (25, 75, 500),   // 50% = 500 steps
        ];

        for (current_pos, target_pos, expected_steps) in test_cases {
            communicator.device_state.current_position = current_pos;
            let calculated_steps = communicator.calculate_steps_needed(target_pos);
            assert_eq!(
                calculated_steps, expected_steps,
                "Formula error: {}% -> {}%, expected {} steps, got {}",
                current_pos, target_pos, expected_steps, calculated_steps
            );
        }
    }

    #[tokio::test]
    async fn test_device_id_filtering() {
        let transport = MockTransport::new();
        let motor = MockMotor::new();
        let mut stepper = Stepper::new(motor);
        let device_mac = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let device_id = 42;
        let edge_id = NodeId::Edge(1);

        stepper.set_max_speed(500.0);
        stepper.set_acceleration(200.0);
        stepper.set_current_position(0);

        let mut communicator = DeviceCommunicator::new(transport, stepper, device_mac, device_id);

        // Test that wrong device IDs are ignored
        let wrong_ids = [41, 43, 0, 100, -1];
        for &wrong_id in &wrong_ids {
            let command_msg = create_edge_command_message(
                edge_id.clone(),
                NodeId::Device(device_mac),
                wrong_id,
                ActuatorCommand::SetWindowPosition(75),
            );

            communicator
                .ble_transport
                .add_message_to_receive(command_msg);
            communicator.handle_edge_message().await.unwrap();
        }

        // No messages should be sent for wrong IDs
        assert_eq!(communicator.ble_transport.get_sent_messages().len(), 0);
        assert_eq!(communicator.device_state.current_position, 0);

        // Test correct device ID is processed
        let correct_msg = create_edge_command_message(
            edge_id,
            NodeId::Device(device_mac),
            device_id,
            ActuatorCommand::SetWindowPosition(75),
        );

        communicator
            .ble_transport
            .add_message_to_receive(correct_msg);
        communicator.handle_edge_message().await.unwrap();

        assert_eq!(communicator.ble_transport.get_sent_messages().len(), 1);
        assert_eq!(communicator.device_state.current_position, 75);
    }

    #[test]
    fn test_battery_level_validation() {
        let transport = MockTransport::new();
        let motor = MockMotor::new();
        let stepper = Stepper::new(motor);
        let device_mac = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let mut communicator = DeviceCommunicator::new(transport, stepper, device_mac, 1);

        // Test automatic clamping of excessive values
        let invalid_values = [101, 150, 255, u8::MAX];
        for &value in &invalid_values {
            communicator.update_battery_level(value);
            assert_eq!(
                communicator.device_state.battery_level, 100,
                "Battery level should be clamped to 100 for input {}",
                value
            );
        }

        // Test battery drain doesn't go below 0
        communicator.device_state.battery_level = 0;
        communicator.simulate_battery_drain();
        assert_eq!(communicator.device_state.battery_level, 0);
    }

    #[tokio::test]
    async fn test_timestamp_monotonicity() {
        let transport = MockTransport::new();
        let motor = MockMotor::new();
        let stepper = Stepper::new(motor);
        let device_mac = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let device_id = 1;
        let edge_id = NodeId::Edge(1);

        let mut communicator = DeviceCommunicator::new(transport, stepper, device_mac, device_id);

        // Rapidly send status requests to test timestamp ordering
        for _ in 0..10 {
            let status_request = create_edge_command_message(
                edge_id.clone(),
                NodeId::Device(device_mac),
                device_id,
                ActuatorCommand::RequestStatus,
            );

            communicator
                .ble_transport
                .add_message_to_receive(status_request);
            communicator.handle_edge_message().await.unwrap();
        }

        let sent_messages = communicator.ble_transport.get_sent_messages();

        // Verify timestamps are monotonically increasing
        let mut last_timestamp = 0u64;
        for (i, message) in sent_messages.iter().enumerate() {
            if let MessagePayload::DeviceReport(DeviceReport::Status {
                relative_timestamp, ..
            }) = &message.payload
            {
                assert!(
                    *relative_timestamp >= last_timestamp,
                    "Non-monotonic timestamp at message {}: {} -> {}",
                    i,
                    last_timestamp,
                    *relative_timestamp
                );
                last_timestamp = *relative_timestamp;
            }
        }
    }
}
