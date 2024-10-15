use alloc::format;
use alloc::string::String;

use embedded_hal_nb::nb::block;
use embedded_hal_nb::serial::Write;

use crate::error::Error;

use super::light_sensor::LightSensor;
use super::temp_sensor::TempSensor;
use super::{SensorConfig, SensorData};

pub const SERIAL_TOPIC: &str = "sensor/data";

fn write_all<S>(serial: &mut S, buffer: &[u8]) -> Result<(), Error>
where
    S: Write<u8, Error = Error>,
{
    for &byte in buffer {
        block!(serial.write(byte))?;
    }
    Ok(())
}

fn send_message<S>(serial: &mut S, topic: &str, payload: &str) -> Result<(), Error>
where
    S: Write<u8, Error = Error>,
{
    let message = format!("{topic}:{payload}\n");
    write_all(serial, message.as_bytes())?;
    block!(serial.flush())?;
    Ok(())
}

pub struct SerialSensorController<LightIo, TempIo, Serial>
where
    LightIo: embedded_io::Read,
    TempIo: embedded_io::Read,
    Serial: Write<u8, Error = Error>,
{
    light_sensor: LightSensor<LightIo>,
    temp_sensor: TempSensor<TempIo>,
    serial: Serial,
    config: SensorConfig,
    last_publish_time: u32, // ms
    sensor_id: String,
    connected: bool,
}

impl<LightIo, TempIo, Serial> SerialSensorController<LightIo, TempIo, Serial>
where
    LightIo: embedded_io::Read,
    TempIo: embedded_io::Read,
    Serial: Write<u8, Error = Error>,
{
    pub fn new(
        light_sensor: LightSensor<LightIo>,
        temp_sensor: TempSensor<TempIo>,
        serial: Serial,
        config: SensorConfig,
    ) -> Self {
        let sensor_id = if config.sensor_id[0] != 0 {
            let mut id = String::with_capacity(24);
            for &b in config.sensor_id.iter() {
                if b == 0 {
                    break;
                }
                id.push(b as char);
            }
            id
        } else {
            String::from("SENSOR-DEFAULT")
        };

        Self {
            light_sensor,
            temp_sensor,
            serial,
            config,
            last_publish_time: 0,
            sensor_id,
            connected: false,
        }
    }

    pub fn init(&mut self) -> Result<(), Error> {
        self.connected = true;
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn process(&mut self, current_time_ms: u32) -> Result<(), Error> {
        if !self.is_connected() {
            self.init()?;
        }

        if current_time_ms - self.last_publish_time >= self.config.publish_interval_ms {
            self.last_publish_time = current_time_ms;

            let lux = self.light_sensor.read_lux()?;

            let temperature = self.temp_sensor.read_temperature()?;

            let data = SensorData::new(self.sensor_id.clone(), lux, temperature);

            let json = data.to_json()?;

            send_message(&mut self.serial, SERIAL_TOPIC, &json)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use core::cell::RefCell;

    use alloc::vec::Vec;

    use embedded_hal_nb::nb;

    use super::super::light_sensor::mock::MockIO as LightMockIO;
    use super::super::temp_sensor::mock::MockIO as TempMockIO;
    use super::*;

    #[derive(Debug)]
    pub enum TestError {
        NotConnected,
        Other,
    }

    impl From<TestError> for Error {
        fn from(err: TestError) -> Self {
            match err {
                TestError::NotConnected => Error::NotConnected,
                TestError::Other => Error::DeviceNotFound,
            }
        }
    }

    pub struct MockSerial {
        sent_data: RefCell<Vec<u8>>,
        connected: bool,
    }

    impl MockSerial {
        pub fn new() -> Self {
            Self {
                sent_data: RefCell::new(Vec::new()),
                connected: false,
            }
        }

        pub fn get_sent_string(&self) -> String {
            let data = self.sent_data.borrow();
            String::from_utf8_lossy(&data).into_owned()
        }

        pub fn connect(&mut self) {
            self.connected = true;
        }
    }

    impl embedded_hal_nb::serial::ErrorType for MockSerial {
        type Error = Error;
    }

    impl Write<u8> for MockSerial {
        fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
            if !self.connected {
                return Err(nb::Error::Other(Error::NotConnected));
            }

            self.sent_data.borrow_mut().push(word);
            Ok(())
        }

        fn flush(&mut self) -> nb::Result<(), Self::Error> {
            if !self.connected {
                return Err(nb::Error::Other(Error::NotConnected));
            }

            Ok(())
        }
    }

    #[test]
    fn test_serial_sensor_controller() {
        let light_io = LightMockIO { value: 2048 };
        let temp_io = TempMockIO { value: 2048 };

        let mut serial = MockSerial::new();
        serial.connect();

        let light_sensor = LightSensor::new(light_io);
        let temp_sensor = TempSensor::new(temp_io);
        let config = SensorConfig::default();
        let mut controller = SerialSensorController::new(light_sensor, temp_sensor, serial, config);

        controller.init().unwrap();
        controller.process(1000).unwrap();

        let sent_data = controller.serial.get_sent_string();

        assert!(sent_data.contains(SERIAL_TOPIC));
        assert!(sent_data.contains("lght"));
        assert!(sent_data.contains("temp"));
    }
}
