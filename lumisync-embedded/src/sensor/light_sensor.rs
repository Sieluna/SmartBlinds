use alloc::vec::Vec;

use embedded_io::Read;

use crate::error::Error;

pub struct LightSensor<IO>
where
    IO: Read,
{
    io_device: IO,
    buffer: Vec<u8>,
}

impl<IO> LightSensor<IO>
where
    IO: Read,
{
    pub fn new(io_device: IO) -> Self {
        Self {
            io_device,
            buffer: Vec::with_capacity(4),
        }
    }

    pub fn read_lux(&mut self) -> Result<f32, Error> {
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

        let lux = self.raw_to_lux(raw_value);

        Ok(lux)
    }

    fn raw_to_lux(&self, raw_value: u16) -> f32 {
        // Map the original value to the 0-1000 range
        const RAW_MAX: f32 = 4095.0;
        const LUX_MAX: f32 = 1000.0;

        (raw_value as f32 / RAW_MAX) * LUX_MAX
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
    fn test_light_sensor() {
        let io = MockIO { value: 2048 }; // 50% of 4095 ~ 500 lux

        let mut sensor = LightSensor::new(io);
        let lux = sensor.read_lux().unwrap();

        let expected = 500.0;
        let tolerance = 0.5;
        assert!(
            (lux - expected).abs() < tolerance,
            "lux value {} not close enough to expected {}",
            lux,
            expected
        );
    }
}
