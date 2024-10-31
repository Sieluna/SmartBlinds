use alloc::vec::Vec;

use embedded_io::Read;

use crate::error::Error;

#[derive(Debug, Clone, Copy)]
pub struct TempSensorCalibration {
    /// Reference resistor value (ohm)
    pub reference_resistor: f32,
    /// NTC thermistor resistance at 25°C (ohm)
    pub r25: f32,
    /// Beta coefficient (K)
    pub beta: f32,
    /// ADC full-scale reference voltage (volt)
    pub reference_voltage: f32,
    /// ADC resolution, typically 10-bit (1024) or 12-bit (4096)
    pub adc_max_value: u16,
}

impl Default for TempSensorCalibration {
    fn default() -> Self {
        Self {
            reference_resistor: 10000.0, // 10K ohm voltage divider resistor
            r25: 10000.0,                // 10K ohm at 25°C
            beta: 3950.0,                // Typical NTC Beta value
            reference_voltage: 3.3,      // 3.3V reference voltage
            adc_max_value: 4095,         // 12-bit ADC
        }
    }
}

pub struct TempSensor<IO>
where
    IO: Read,
{
    io_device: IO,
    buffer: Vec<u8>,
    calibration: TempSensorCalibration,
}

impl<IO> TempSensor<IO>
where
    IO: Read,
{
    pub fn new(io_device: IO) -> Self {
        Self {
            io_device,
            buffer: Vec::with_capacity(4),
            calibration: TempSensorCalibration::default(),
        }
    }

    pub fn with_calibration(io_device: IO, calibration: TempSensorCalibration) -> Self {
        Self {
            io_device,
            buffer: Vec::with_capacity(4),
            calibration,
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

        // Check if raw value is within valid range
        if raw_value > self.calibration.adc_max_value {
            return Err(Error::SensorReadingOutOfRange);
        }

        let temperature = self.raw_to_temperature(raw_value);

        // Check if converted temperature is within reasonable range
        if !(-50.0..=150.0).contains(&temperature) {
            return Err(Error::SensorReadingOutOfRange);
        }

        Ok(temperature)
    }

    fn raw_to_temperature(&self, raw_value: u16) -> f32 {
        // Calculate temperature using Beta equation
        // 1. Calculate NTC resistance
        let adc_value = raw_value as f32;
        let adc_max = self.calibration.adc_max_value as f32;

        // Prevent division by zero
        if adc_value == 0.0 {
            return 150.0; // Return maximum temperature value
        }

        // Voltage divider formula: NTC_R = R_ref * (ADC_max/ADC_value - 1)
        let ntc_resistance = self.calibration.reference_resistor * (adc_max / adc_value - 1.0);

        if ntc_resistance <= 0.0 {
            return 150.0; // Invalid resistance value, return maximum temperature
        }

        // 2. Calculate temperature using Beta equation (in Kelvin)
        // 1/T = 1/T0 + (1/B) * ln(R/R0)
        // T0 = 298.15K (25°C)
        const T0: f32 = 298.15; // 25°C in Kelvin

        let ln_ratio = libm::logf(ntc_resistance / self.calibration.r25);
        let inv_temp = (1.0 / T0) + (1.0 / self.calibration.beta) * ln_ratio;

        // Prevent division by zero
        if inv_temp <= 0.0 {
            return 150.0;
        }

        // Convert to Celsius (K - 273.15 = °C)
        let celsius = (1.0 / inv_temp) - 273.15;

        celsius.clamp(-50.0, 150.0) // Limit to reasonable range
    }

    pub fn set_calibration(&mut self, calibration: TempSensorCalibration) {
        self.calibration = calibration;
    }

    pub fn get_calibration(&self) -> TempSensorCalibration {
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
    fn test_temp_sensor() {
        const CASES: &[(u16, f32)] = &[
            (662, -10.0),
            (980, 0.0),
            (2048, 25.0),
            (3013, 50.0),
            (3564, 75.0),
        ];

        for &(raw, expected) in CASES {
            let io = MockIO { value: raw };
            let mut sensor = TempSensor::new(io);
            let t = sensor.read_temperature().unwrap();
            assert!(
                (t - expected).abs() < 3.0, // ±3 ℃
                "T {:.2} ℃ not within ±3 ℃ of {:.2}",
                t,
                expected
            );
        }
    }
}
