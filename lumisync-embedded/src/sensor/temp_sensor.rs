use alloc::vec::Vec;

use embedded_io::Read;

use crate::error::Error;

pub struct TempSensor<IO>
where
    IO: Read,
{
    io_device: IO,
    buffer: Vec<u8>,
}

impl<IO> TempSensor<IO>
where
    IO: Read,
{
    pub fn new(io_device: IO) -> Self {
        Self {
            io_device,
            buffer: Vec::with_capacity(4),
        }
    }

    pub fn read_temperature(&mut self) -> Result<f32, Error> {
        self.buffer.clear();
        self.buffer.resize(4, 0);

        let read_count = self
            .io_device
            .read(&mut self.buffer)
            .map_err(|_| Error::DeviceNotFound)?;

        if read_count < 2 {
            return Err(Error::DeviceNotFound);
        }

        let raw_value = u16::from_be_bytes([self.buffer[0], self.buffer[1]]);
        let temperature = self.raw_to_temperature(raw_value);

        Ok(temperature)
    }

    fn raw_to_temperature(&self, raw_value: u16) -> f32 {
        const RAW_MAX: f32 = 4095.0;

        // Assume that the original value 0 corresponds to -20℃, and the maximum value corresponds to 50℃
        const TEMP_MIN: f32 = -20.0;
        const TEMP_MAX: f32 = 50.0;

        TEMP_MIN + (raw_value as f32 / RAW_MAX) * (TEMP_MAX - TEMP_MIN)
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;

    #[derive(Debug)]
    pub struct MockIO {
        pub value: u16,
    }

    impl embedded_io::ErrorType for MockIO {
        type Error = embedded_io::ErrorKind;
    }

    impl Read for MockIO {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            if buf.len() < 2 {
                return Ok(0);
            }

            let bytes = self.value.to_be_bytes();
            buf[0] = bytes[0];
            buf[1] = bytes[1];

            Ok(2)
        }
    }

    #[test]
    fn test_temp_sensor() {
        let io = MockIO { value: 2048 }; // 50% of 4095 ~ 15 degrees

        let mut sensor = TempSensor::new(io);
        let temp = sensor.read_temperature().unwrap();

        let expected = 15.0;
        let tolerance = 0.5;
        assert!(
            (temp - expected).abs() < tolerance,
            "temperature value {} not close enough to expected {}",
            temp,
            expected
        );
    }
}
