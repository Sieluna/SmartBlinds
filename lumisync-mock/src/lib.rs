use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use rumqttd::local::LinkTx;
use serde::{Deserialize, Serialize};
use serde_json::{json, to_vec};
use time::OffsetDateTime;

use crate::broker::MockBroker;
use crate::command::CommandHandler;
use crate::settings::Settings;
use crate::simulate::SensorSimulator;

pub mod broker;
pub mod command;
pub mod settings;
mod simulate;

#[derive(Serialize, Deserialize, Debug)]
pub struct SensorPayload {
    id: String,
    airp: Option<f32>, // Air pressure in hPa
    lght: Option<i32>, // Light level in lux
    temp: Option<f32>, // Temperature in degrees Celsius
    humd: Option<f32>, // Humidity as percentage (0-100%)
}

pub async fn run(settings: &Arc<Settings>) {
    let gateway = Arc::new(settings.gateway.clone());
    let topic = Arc::new(gateway.topic.clone());
    let mock = Arc::new(settings.mock.clone());

    // Create MQTT topic prefix
    let prefix = format!(
        "cloudext/{}/{}/{}/{}",
        &topic.prefix_type, &topic.prefix_mode, &topic.prefix_country, &mock.group_name,
    );

    let broker = MockBroker::new(&gateway).expect("Failed to create broker");
    let mut command_handler = CommandHandler::new();
    let mut link_tx = command_handler
        .start_sensor_command_processor(&broker)
        .await;

    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let mut mock_index = 0;
    let mut simulator = SensorSimulator::new();

    loop {
        tokio::select! {
            Some((data, sensor_index)) = command_handler.cmd_rx.recv() => {
                // Calculate time of day as fraction (0.0-1.0)
                let now = OffsetDateTime::now_utc();
                let time = now.time();
                let seconds_since_midnight = time.hour() as u64 * 3600
                                           + time.minute() as u64 * 60
                                           + time.second() as u64;
                let day_fraction = seconds_since_midnight as f64 / 86400.0;

                publish_env_message(
                    &mut link_tx,
                    sensor_index,
                    &prefix,
                    &mock.group_name,
                    data,
                    day_fraction,
                    &mut simulator,
                )
                .await
                .unwrap();
            },

            // Handle periodic updates
            _ = interval.tick() => {
                const INTERVAL_COUNT: u32 = 180; // 30-minute cycle with 10-second intervals

                // Simulate cyclical pattern for demo purposes
                let day_fraction = (mock_index % INTERVAL_COUNT) as f64 / INTERVAL_COUNT as f64;

                let data = SensorPayload {
                    id: "SENSOR-MOCK".to_string(),
                    airp: None,
                    lght: None,
                    temp: None,
                    humd: None,
                };

                publish_env_message(
                    &mut link_tx,
                    mock_index,
                    &prefix,
                    &mock.group_name,
                    data,
                    day_fraction,
                    &mut simulator,
                )
                .await
                .unwrap();

                mock_index += 1;
            }
        }
    }
}

async fn publish_env_message(
    client: &mut LinkTx,
    index: u32,
    prefix: &str,
    customer_id: &str,
    data: SensorPayload,
    day_fraction: f64,
    simulator: &mut SensorSimulator,
) -> Result<(), Box<dyn Error>> {
    let SensorPayload {
        id,
        airp,
        lght,
        temp,
        humd,
    } = data;

    let (air_pressure, temperature, humidity, light) = simulator.generate(day_fraction);

    let airp_value = airp.unwrap_or(air_pressure as f32);
    let temp_value = temp.unwrap_or(temperature as f32);
    let humd_value = humd.unwrap_or(humidity as f32);
    let lght_value = lght.unwrap_or(light as i32);

    let env_msg = json!({
        // Message Id
        "mId": index,
        // Message Timestamp
        "mTs": OffsetDateTime::now_utc().unix_timestamp(),
        // Air Pressure (hPa)
        "airp": airp_value,
        // Light Level (lux)
        "lght": lght_value,
        // Temperature (°C)
        "temp": temp_value,
        // Humidity (0-100%)
        "humd": humd_value,
        // Sensor ID
        "sId": id,
        // Customer key
        "cId": customer_id
    });

    let env_topic = format!("{prefix}/{customer_id}/{id}/{index}");

    tracing::debug!("Topic: {}, Send: {}", &env_topic, &env_msg);

    client.publish(env_topic, to_vec(&env_msg)?)?;

    Ok(())
}
