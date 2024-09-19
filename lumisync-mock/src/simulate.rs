use rand::Rng;
use rand_distr::{Distribution, Normal, LogNormal};
use std::f64::consts;

// Environmental constants (hPa, °C, %, lux)
const STANDARD_PRESSURE: f64 = 1013.25;
const BASE_TEMPERATURE: f64 = 22.0;
const BASE_HUMIDITY: f64 = 50.0;
const MAX_SUNLIGHT: f64 = 1000.0;
const MIN_DAYLIGHT: f64 = 150.0;
const MAX_MOONLIGHT: f64 = 1.0;
const MIN_NIGHT: f64 = 0.05;

// Day cycle fractions (24-hour)
const SUNRISE_START: f64 = 0.2;   // 4:48 AM
const SUNRISE_END: f64 = 0.25;    // 6:00 AM
const SUNSET_START: f64 = 0.75;   // 6:00 PM
const SUNSET_END: f64 = 0.8;      // 7:12 PM

// Variation parameters
const TEMP_DAY_VARIATION: f64 = 4.0;
const PRESSURE_VARIATION: f64 = 2.5;
const HUMIDITY_DAY_VARIATION: f64 = 20.0;
const WEATHER_CYCLE_DAYS: f64 = 3.0;

// Noise parameters
const AIRP_NOISE: f64 = 0.15;
const TEMP_NOISE: f64 = 0.1;
const HUMD_NOISE: f64 = 0.7;
const LIGHT_NOISE: f64 = 0.05;

// System dynamics
const TEMP_INERTIA: f64 = 0.92;
const PRESSURE_INERTIA: f64 = 0.95;
const HUMIDITY_RESPONSE: f64 = 0.85;

pub struct SensorSimulator {
    // Previous states: pressure(hPa), temp(°C), humidity(%), clouds(0-1), weather(-1~1)
    last_airp: f64,
    last_temp: f64,
    last_humd: f64,
    last_cloud_cover: f64,
    last_weather_trend: f64,
    
    day_offset: f64,
    rng: rand::rngs::ThreadRng,
    airp_noise: Normal<f64>,
    temp_noise: Normal<f64>,
    humd_noise: Normal<f64>,
    light_noise: LogNormal<f64>,
    weather_noise: Normal<f64>,
}

impl Default for SensorSimulator {
    fn default() -> Self {
        let mut rng = rand::rng();
        Self {
            last_airp: STANDARD_PRESSURE + rng.random_range(-3.0..3.0),
            last_temp: BASE_TEMPERATURE + rng.random_range(-1.5..1.5),
            last_humd: BASE_HUMIDITY + rng.random_range(-10.0..10.0),
            last_cloud_cover: rng.random_range(0.1..0.5),
            last_weather_trend: rng.random_range(-0.5..0.5),
            day_offset: 0.0,
            rng,
            airp_noise: Normal::new(0.0, AIRP_NOISE).unwrap(),
            temp_noise: Normal::new(0.0, TEMP_NOISE).unwrap(),
            humd_noise: Normal::new(0.0, HUMD_NOISE).unwrap(),
            light_noise: LogNormal::new(-1.0, LIGHT_NOISE).unwrap(),
            weather_noise: Normal::new(0.0, 0.05).unwrap(),
        }
    }
}

impl SensorSimulator {
    pub fn new() -> Self { Self::default() }

    pub fn advance_day(&mut self) {
        self.day_offset += 1.0;
        let drift = self.weather_noise.sample(&mut self.rng);
        self.last_weather_trend = (self.last_weather_trend + drift).clamp(-1.0, 1.0) * 0.85;
    }

    /// Generates sensor data for given time fraction (0.0-1.0 represents current day)
    pub fn generate(&mut self, day_fraction: f64) -> (f64, f64, f64, f64) {
        let absolute_day = self.day_offset + day_fraction;
        let weather = self.update_weather(absolute_day);
        let clouds = self.update_clouds(day_fraction, weather);
        
        let pressure = self.simulate_pressure(absolute_day, weather);
        let temp = self.simulate_temp(day_fraction, weather, clouds);
        let humidity = self.simulate_humidity(absolute_day, temp);
        let light = self.simulate_light(day_fraction, weather, clouds);
        
        (pressure, temp, humidity, light)
    }

    fn update_weather(&mut self, absolute_day: f64) -> f64 {
        let base = (absolute_day / WEATHER_CYCLE_DAYS * consts::TAU).sin();
        let drift = self.weather_noise.sample(&mut self.rng) * 0.1;
        self.last_weather_trend = (self.last_weather_trend + drift + base * 0.03)
            .clamp(-1.0, 1.0);
        self.last_weather_trend
    }

    fn update_clouds(&mut self, day_fraction: f64, weather: f64) -> f64 {
        let diurnal = ((day_fraction - 0.5) * 6.0 * consts::PI).cos() * 0.15;
        let target = 0.4 - weather * 0.3 + diurnal;
        let noise = self.temp_noise.sample(&mut self.rng) * 0.1;
        
        self.last_cloud_cover = (self.last_cloud_cover * 0.9 + target * 0.1 + noise)
            .clamp(0.05, 0.95);
        self.last_cloud_cover
    }

    fn simulate_pressure(&mut self, absolute_day: f64, weather: f64) -> f64 {
        let diurnal = Self::diurnal_pressure(absolute_day % 1.0);
        let base = STANDARD_PRESSURE + diurnal + weather * PRESSURE_VARIATION * 2.0;
        let noise = self.airp_noise.sample(&mut self.rng);
        
        self.last_airp = self.last_airp.mul_add(PRESSURE_INERTIA, base * (1.0 - PRESSURE_INERTIA)) + noise;
        self.last_airp
    }

    fn simulate_temp(&mut self, day_fraction: f64, weather: f64, clouds: f64) -> f64 {
        let diurnal = Self::diurnal_temperature(day_fraction);
        let base = BASE_TEMPERATURE + diurnal * (1.0 - clouds * 0.5) + weather * 3.0;
        let noise = self.temp_noise.sample(&mut self.rng);
        
        self.last_temp = self.last_temp.mul_add(TEMP_INERTIA, base * (1.0 - TEMP_INERTIA)) + noise;
        self.last_temp
    }

    fn simulate_humidity(&mut self, absolute_day: f64, temp: f64) -> f64 {
        let weather = self.last_weather_trend;
        let hour_effect = ((absolute_day % 1.0 - 0.05) * consts::TAU).cos() * HUMIDITY_DAY_VARIATION * 0.3;
        let target = BASE_HUMIDITY + (BASE_TEMPERATURE - temp) * 2.5 - weather * 10.0 + hour_effect;
        let noise = self.humd_noise.sample(&mut self.rng);
        
        let humidity = self.last_humd.mul_add(HUMIDITY_RESPONSE, target * (1.0 - HUMIDITY_RESPONSE)) + noise;
        self.last_humd = humidity.clamp(10.0, 95.0);
        self.last_humd
    }

    fn simulate_light(&self, day_fraction: f64, weather: f64, clouds: f64) -> f64 {
        let base = Self::base_illumination(day_fraction);
        let cloud_factor = 1.0 - clouds.powf(1.5);
        let weather_factor = 1.0 + weather * 0.3;
        
        let light = base * cloud_factor * weather_factor * self.light_noise.sample(&mut self.rng.clone());
        let min_light = if is_daytime(day_fraction) { MIN_DAYLIGHT } else { MIN_NIGHT };
        light.max(min_light * cloud_factor)
    }

    fn diurnal_pressure(day_fraction: f64) -> f64 {
        let radians = day_fraction * consts::TAU * 2.0;
        (radians.sin() * 0.3 + radians.cos() * 0.7) * 0.5
    }

    fn diurnal_temperature(day_fraction: f64) -> f64 {
        let phase = (day_fraction - 0.15) * consts::TAU;
        -phase.cos() * TEMP_DAY_VARIATION
    }

    fn base_illumination(day_fraction: f64) -> f64 {
        match day_fraction {
            t if t < SUNRISE_START || t > SUNSET_END => {
                let moon_phase = ((t + 0.5) % 1.0) * consts::TAU;
                MAX_MOONLIGHT * moon_phase.sin().max(0.0) + MIN_NIGHT
            },
            t if t <= SUNRISE_END => {
                let progress = (t - SUNRISE_START) / (SUNRISE_END - SUNRISE_START);
                let factor = (progress.powi(2) * (3.0 - 2.0 * progress)).powf(1.2);
                MIN_DAYLIGHT + factor * (MAX_SUNLIGHT - MIN_DAYLIGHT)
            },
            t if t >= SUNSET_START => {
                let progress = (t - SUNSET_START) / (SUNSET_END - SUNSET_START);
                let factor = (1.0 - progress.powi(2) * (3.0 - 2.0 * progress)).powf(1.2);
                MIN_DAYLIGHT + factor * (MAX_SUNLIGHT - MIN_DAYLIGHT)
            },
            _ => {
                let noon_offset = (day_fraction - 0.5) * 2.0;
                MAX_SUNLIGHT * (1.0 - 0.3 * noon_offset.powi(2))
            }
        }
    }
}

fn is_daytime(t: f64) -> bool {
    (SUNRISE_START..=SUNSET_END).contains(&t)
}
