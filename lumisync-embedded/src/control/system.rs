use alloc::boxed::Box;

use async_trait::async_trait;
use heapless::{FnvIndexMap, Vec};

use super::{ControlConfig, DeviceControlState, PIDController, ZoneStrategy};

use crate::error::Error;
use crate::types::*;

const MAX_CONTROL_LOOPS: usize = 16;

#[derive(Debug)]
pub struct ControlSystemState {
    pub active_zones: Vec<u8, 4>,
    pub device_states: FnvIndexMap<DeviceId, DeviceControlState, 16>,
}

#[async_trait]
pub trait ControlSystem {
    fn new(config: ControlConfig) -> Self;

    async fn process_sensor_data(
        &mut self,
        sensor_data: SensorData,
        sensor_id: DeviceId,
    ) -> Result<(), Error>;

    async fn handle_network_message(&mut self, message: NetworkMessage) -> Result<(), Error>;

    async fn handle_advertisement(&mut self, adv: AdvertisementData) -> Result<(), Error>;

    fn find_sensor_for_data(&self, data: &SensorData) -> Option<DeviceId>;

    async fn maintain_control_system(&mut self) -> Result<(), Error>;
}

pub struct DefaultControlSystem {
    config: ControlConfig,
    state: ControlSystemState,
    controllers: FnvIndexMap<DeviceId, PIDController, MAX_CONTROL_LOOPS>,
    strategies: FnvIndexMap<u8, ZoneStrategy, 4>,
}

#[async_trait]
impl ControlSystem for DefaultControlSystem {
    fn new(config: ControlConfig) -> Self {
        Self {
            config,
            state: ControlSystemState {
                active_zones: Vec::new(),
                device_states: FnvIndexMap::new(),
            },
            controllers: FnvIndexMap::new(),
            strategies: FnvIndexMap::new(),
        }
    }

    async fn process_sensor_data(
        &mut self,
        sensor_data: SensorData,
        sensor_id: DeviceId,
    ) -> Result<(), Error> {
        // Find the zone strategy for this sensor
        for (zone_id, strategy) in self.strategies.iter() {
            // Update PID controllers for windows in the same zone
            for (device_id, controller) in self.controllers.iter_mut() {
                if let Some(device_state) = self.state.device_states.get(device_id) {
                    // Calculate control output
                    let output = controller.update(
                        strategy.target_light as f32,
                        sensor_data.light as f32,
                        0.1, // dt in seconds
                    );

                    // Convert PID output to window position
                    let position = (output as i8).clamp(-100, 100);

                    // Send control command
                    let command = ControlCommand::SetPosition {
                        window_id: *device_id,
                        position,
                    };

                    // Here should call the concrete implementation to send the command
                    // self.send_command(command).await?;
                }
            }
        }
        Ok(())
    }

    async fn handle_network_message(&mut self, message: NetworkMessage) -> Result<(), Error> {
        match message {
            NetworkMessage::Response(ResponseData::SensorData(data)) => {
                if let Some(sensor_id) = self.find_sensor_for_data(&data) {
                    self.process_sensor_data(data, sensor_id).await?;
                }
            }
            NetworkMessage::Response(ResponseData::WindowState(_state)) => {
                // Update window state
                // Here should call the concrete implementation to update the state
            }
            NetworkMessage::Advertisement(adv) => {
                self.handle_advertisement(adv).await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_advertisement(&mut self, adv: AdvertisementData) -> Result<(), Error> {
        // Update device state
        let device_state = DeviceControlState {
            device_id: adv.device_id,
            last_command: None,
            last_update: 0,
            error_count: 0,
        };

        self.state
            .device_states
            .insert(adv.device_id, device_state)
            .map_err(|_| Error::DeviceNotFound)?;

        // Initialize controller if needed
        if matches!(adv.node_type, NodeType::Window)
            && !self.controllers.contains_key(&adv.device_id)
        {
            let controller = PIDController::new(Default::default());
            self.controllers
                .insert(adv.device_id, controller)
                .map_err(|_| Error::DeviceNotFound)?;
        }

        Ok(())
    }

    fn find_sensor_for_data(&self, _data: &SensorData) -> Option<DeviceId> {
        // TODO: Implement sensor matching
        None
    }

    async fn maintain_control_system(&mut self) -> Result<(), Error> {
        // Here should call the concrete implementation to maintain the control system
        Ok(())
    }
}
