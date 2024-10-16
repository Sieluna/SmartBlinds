use alloc::vec::Vec;

use embedded_io::Read;

use crate::error::Error;

#[derive(Debug, Clone, Copy)]
pub struct LightSensorCalibration {
    /// Reference resistor value (ohms)
    pub reference_resistor: f32,
    /// Sensor resistance at 10 lux (ohms)
    pub r10: f32,
    /// ADC reference voltage (volts)
    pub reference_voltage: f32,
    /// ADC resolution, typically 10-bit (1024) or 12-bit (4096)
    pub adc_max_value: u16,
}

impl Default for LightSensorCalibration {
    fn default() -> Self {
        Self {
            reference_resistor: 10000.0, // 10K ohm voltage divider
            r10: 10000.0,                // Resistance at 10 lux
            reference_voltage: 3.3,      // 3.3V reference voltage
            adc_max_value: 4095,         // 12-bit ADC
        }
    }
}

pub struct LightSensor<IO>
where
    IO: Read,
{
    io_device: IO,
    buffer: Vec<u8>,
    calibration: LightSensorCalibration,
}

impl<IO> LightSensor<IO>
where
    IO: Read,
{
    pub fn new(io_device: IO) -> Self {
        Self {
            io_device,
            buffer: Vec::with_capacity(4),
            calibration: LightSensorCalibration::default(),
        }
    }

    pub fn with_calibration(io_device: IO, calibration: LightSensorCalibration) -> Self {
        Self {
            io_device,
            buffer: Vec::with_capacity(4),
            calibration,
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

        // Check if raw value is within valid range
        if raw_value > self.calibration.adc_max_value {
            return Err(Error::SensorReadingOutOfRange);
        }

        let lux = self.raw_to_lux(raw_value);

        // Check if result is within valid range (prevent negative or extreme values)
        if !(0.0..=100000.0).contains(&lux) {
            return Err(Error::SensorReadingOutOfRange);
        }

        Ok(lux)
    }

    fn raw_to_lux(&self, raw_value: u16) -> f32 {
        // Non-linear conversion formula
        // 1. Calculate LDR resistance
        // Voltage divider formula: LDR_R = R_ref * (ADC_max/ADC_value - 1)
        let adc_ratio = self.calibration.adc_max_value as f32 / raw_value as f32;
        let ldr_resistance = self.calibration.reference_resistor * (adc_ratio - 1.0);

        if ldr_resistance <= 0.0 {
            return 100000.0; // Maximum value for extremely bright conditions
        }

        // 2. Calculate light intensity (lux) using power law formula
        // Typical formula: Lux = (R10/R)^gamma, where R10 is resistance at 10 lux, gamma typically 1.5
        let gamma: f32 = 1.5;
        let lux = 10.0 * libm::powf(self.calibration.r10 / ldr_resistance, gamma);

        lux.max(0.0).min(100000.0) // Limit to valid range
    }

    pub fn set_calibration(&mut self, calibration: LightSensorCalibration) {
        self.calibration = calibration;
    }

    pub fn get_calibration(&self) -> LightSensorCalibration {
        self.calibration
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
        const CASES: &[(u16, f32)] = &[(67, 0.02), (512, 0.5), (2048, 10.0), (3561, 172.0)];

        for &(raw, expected) in CASES {
            let io = MockIO { value: raw };
            let mut sensor = LightSensor::new(io);
            let lux = sensor.read_lux().unwrap();
            assert!(
                (lux - expected).abs() / expected < 0.20, // ±20 %
                "lux {:.2} not within 20 % of {:.2}",
                lux,
                expected
            );
        }
    }
}
