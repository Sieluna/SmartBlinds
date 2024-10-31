use embassy_time::{Duration, Timer};
use lumisync_api::message::*;
use lumisync_api::{Id, SensorData};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::communication::MessageTransport;
use crate::protocol::message::MessageBuilder;
use crate::stepper::{Motor, Stepper};
use crate::{Error, Result};

use super::status::DeviceStatus;

pub struct DeviceCommunicator<T, M>
where
    T: MessageTransport,
    M: Motor,
{
    /// BLE transport layer (for communicating with Edge)
    ble_transport: T,
    /// Motor controller
    motor: M,
    /// Message builder
    message_builder: MessageBuilder,
    /// Current device node ID
    node_id: NodeId,
    /// Device status
    device_state: DeviceStatus,
}

impl<T, M> DeviceCommunicator<T, M>
where
    T: MessageTransport,
    M: Motor + Clone,
{
    pub fn new(ble_transport: T, motor: M, device_mac: [u8; 6], device_id: Id) -> Self {
        Self {
            ble_transport,
            motor,
            message_builder: MessageBuilder::new(),
            node_id: NodeId::Device(device_mac),
            device_state: DeviceStatus {
                device_id,
                ..Default::default()
            },
        }
    }

    /// Handle messages from Edge
    pub async fn handle_edge_message(&mut self) -> Result<()> {
        if let Ok(Some(message)) = self.ble_transport.receive_message().await {
            match &message.payload {
                MessagePayload::EdgeCommand(edge_cmd) => {
                    self.process_edge_command(edge_cmd, &message.header.source)
                        .await?;
                }
                _ => {
                    // Ignore other message types
                }
            }
        }
        Ok(())
    }

    /// Process Edge commands
    async fn process_edge_command(&mut self, command: &EdgeCommand, source: &NodeId) -> Result<()> {
        match command {
            EdgeCommand::Actuator {
                actuator_id,
                sequence: _,
                command,
            } => {
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

    /// Set window position
    async fn set_window_position(&mut self, position: u8) -> Result<()> {
        let position = position.min(100); // Ensure position doesn't exceed 100%

        self.device_state.target_position = position;
        self.device_state.is_moving = true;
        self.device_state.error_code = 0;

        // Calculate steps needed for target position
        let steps_needed = self.calculate_steps_needed(position);

        self.motor.enable();

        // Create stepper controller
        let mut stepper = Stepper::new(self.motor.clone());

        // Configure stepper parameters
        stepper.set_max_speed(500.0); // Steps per second
        stepper.set_acceleration(200.0); // Steps per second squared

        // Set initial position
        stepper.set_current_position(0);

        // Move to target position
        stepper.move_to(steps_needed);

        // Run asynchronously until completion
        await_stepper_completion(&mut stepper).await;

        // Graceful shutdown - gradually decelerate to stop
        gradual_shutdown(&mut stepper, &mut self.motor).await;

        self.device_state.current_position = position;
        self.device_state.is_moving = false;

        Ok(())
    }

    /// Calculate steps needed
    fn calculate_steps_needed(&self, target_position: u8) -> i64 {
        let current = self.device_state.current_position as i64;
        let target = target_position as i64;

        // Stepper motor configuration parameters
        const STEPS_PER_REVOLUTION: i64 = 200; // 1.8 degree stepper motor
        const GEAR_RATIO: i64 = 10; // Reduction ratio
        const FULL_RANGE_DEGREES: i64 = 180; // Angle range from fully open to fully closed

        // Calculate steps needed from current position to target position
        let position_diff_percent = target - current;
        let position_diff_degrees = (position_diff_percent * FULL_RANGE_DEGREES) / 100;

        (position_diff_degrees * STEPS_PER_REVOLUTION * GEAR_RATIO) / 360
    }

    /// Emergency stop
    async fn emergency_stop(&mut self) -> Result<()> {
        // Directly disable motor in emergency
        self.motor.disable();

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
        let message = self.message_builder.device_status(
            self.node_id.clone(),
            target.clone(),
            self.device_state.device_id,
            self.device_state.current_position,
            self.device_state.battery_level,
            self.device_state.error_code,
        );

        self.ble_transport
            .send_message(&message)
            .await
            .map_err(|_| Error::NetworkError)?;

        Ok(())
    }

    /// Send health status
    async fn send_health_status(&mut self, target: &NodeId) -> Result<()> {
        let message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: self.node_id.clone(),
                target: target.clone(),
            },
            payload: MessagePayload::DeviceReport(DeviceReport::HealthStatus {
                device_id: self.device_state.device_id,
                cpu_usage: 25.0,
                memory_usage: 45.0,
                battery_level: self.device_state.battery_level,
                uptime: 3600,         // 1 hour
                signal_strength: -50, // dBm
            }),
        };

        self.ble_transport
            .send_message(&message)
            .await
            .map_err(|_| Error::NetworkError)?;

        Ok(())
    }

    /// Send sensor data
    async fn send_sensor_data(&mut self, target: &NodeId) -> Result<()> {
        let message = Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source: self.node_id.clone(),
                target: target.clone(),
            },
            payload: MessagePayload::DeviceReport(DeviceReport::SensorData {
                actuator_id: self.device_state.device_id,
                sensor_data: SensorData {
                    temperature: 23.5,
                    illuminance: 1200,
                    humidity: 65.0,
                    timestamp: OffsetDateTime::now_utc(),
                },
            }),
        };

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

    /// Update battery level
    pub fn update_battery_level(&mut self, level: u8) {
        self.device_state.battery_level = level.min(100);
    }

    /// Simulate battery drain
    pub fn simulate_battery_drain(&mut self) {
        if self.device_state.battery_level > 0 {
            self.device_state.battery_level = self.device_state.battery_level.saturating_sub(1);
        }
    }
}

/// Wait for stepper motor to complete movement
async fn await_stepper_completion<M: Motor>(stepper: &mut Stepper<M>) {
    let mut last_position = stepper.get_current_position();
    let target = stepper.get_target_position();

    while stepper.get_speed() != 0.0 || stepper.get_current_position() != target {
        // Run one step
        let current_time =
            core::time::Duration::from_micros(embassy_time::Instant::now().as_micros());
        stepper.run(current_time);

        // Check if stuck (position unchanged for a period)
        if stepper.get_current_position() == last_position {
            // Increment counter and break if stuck too long
            // Should add counter logic in actual application
        }
        last_position = stepper.get_current_position();

        // Yield execution
        Timer::after(Duration::from_millis(1)).await;
    }
}

/// Implement graceful shutdown - gradually decelerate then power off
async fn gradual_shutdown<M: Motor>(stepper: &mut Stepper<M>, motor: &mut M) {
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
    motor.disable();
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

        fn clear_sent_messages(&mut self) {
            self.messages.clear();
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
        actuator_id: Id,
        command: ActuatorCommand,
    ) -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
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

    fn create_health_status_request(source: NodeId, target: NodeId, actuator_id: Id) -> Message {
        Message {
            header: MessageHeader {
                id: Uuid::new_v4(),
                timestamp: OffsetDateTime::now_utc(),
                priority: Priority::Regular,
                source,
                target,
            },
            payload: MessagePayload::EdgeCommand(EdgeCommand::RequestHealthStatus { actuator_id }),
        }
    }

    fn is_device_status_report(message: &Message, device_id: Id) -> bool {
        if let MessagePayload::DeviceReport(DeviceReport::Status { actuator_id, .. }) =
            &message.payload
        {
            return *actuator_id == device_id;
        }
        false
    }

    fn is_health_status_report(message: &Message, device_id: Id) -> bool {
        if let MessagePayload::DeviceReport(DeviceReport::HealthStatus { device_id: id, .. }) =
            &message.payload
        {
            return *id == device_id;
        }
        false
    }

    #[test]
    fn test_calculate_steps_needed() {
        let transport = MockTransport::new();
        let motor = MockMotor::new();
        let device_mac = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let communicator = DeviceCommunicator::new(transport, motor, device_mac, 1);

        let steps = communicator.calculate_steps_needed(50);
        assert_eq!(steps, 500);

        let steps = communicator.calculate_steps_needed(0);
        assert_eq!(steps, 0);

        let steps = communicator.calculate_steps_needed(100);
        assert_eq!(steps, 1000);

        let transport = MockTransport::new();
        let motor = MockMotor::new();
        let mut communicator = DeviceCommunicator::new(transport, motor, device_mac, 1);
        communicator.device_state.current_position = 50;

        let steps = communicator.calculate_steps_needed(30);
        assert_eq!(steps, -200);
    }

    #[tokio::test]
    async fn test_process_edge_commands() {
        let transport = MockTransport::new();
        let motor = MockMotor::new();
        let device_mac = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let device_id = 1;
        let edge_id = NodeId::Edge(1);

        let mut communicator = DeviceCommunicator::new(transport, motor, device_mac, device_id);

        communicator.device_state.is_moving = true;
        communicator.device_state.current_position = 50;
        communicator.device_state.target_position = 80;

        let emergency_message = create_edge_command_message(
            edge_id.clone(),
            NodeId::Device(device_mac),
            device_id,
            ActuatorCommand::EmergencyStop,
        );
        communicator
            .ble_transport
            .add_message_to_receive(emergency_message);
        communicator.handle_edge_message().await.unwrap();

        assert_eq!(communicator.device_state.is_moving, false);
        assert_eq!(communicator.device_state.target_position, 50);

        let sent_messages = communicator.ble_transport.get_sent_messages();
        assert_eq!(sent_messages.len(), 1);
        assert!(is_device_status_report(&sent_messages[0], device_id));

        communicator.ble_transport.clear_sent_messages();

        let health_message =
            create_health_status_request(edge_id.clone(), NodeId::Device(device_mac), device_id);
        communicator
            .ble_transport
            .add_message_to_receive(health_message);
        communicator.handle_edge_message().await.unwrap();

        let sent_messages = communicator.ble_transport.get_sent_messages();
        assert_eq!(sent_messages.len(), 1);
        assert!(is_health_status_report(&sent_messages[0], device_id));
    }

    #[tokio::test]
    async fn test_position_limit_handling() {
        let transport = MockTransport::new();
        let motor = MockMotor::new();
        let device_mac = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let device_id = 1;

        let mut communicator = DeviceCommunicator::new(transport, motor, device_mac, device_id);

        communicator.set_window_position(150).await.unwrap();

        assert_eq!(communicator.device_state.current_position, 100);
        assert_eq!(communicator.device_state.target_position, 100);
    }

    #[tokio::test]
    async fn test_ignore_irrelevant_commands() {
        let transport = MockTransport::new();
        let motor = MockMotor::new();
        let device_mac = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let device_id = 1;
        let edge_id = NodeId::Edge(1);

        let other_device_message = create_edge_command_message(
            edge_id.clone(),
            NodeId::Device(device_mac),
            2,
            ActuatorCommand::SetWindowPosition(75),
        );

        let mut transport_clone = transport.clone();
        transport_clone.add_message_to_receive(other_device_message);

        let mut communicator =
            DeviceCommunicator::new(transport_clone, motor, device_mac, device_id);

        communicator.handle_edge_message().await.unwrap();

        assert_eq!(communicator.device_state.current_position, 0);
        assert_eq!(communicator.device_state.target_position, 0);

        let sent_messages = communicator.ble_transport.get_sent_messages();
        assert_eq!(sent_messages.len(), 0);
    }
}
