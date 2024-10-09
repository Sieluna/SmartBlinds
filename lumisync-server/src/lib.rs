pub mod app;
pub mod configs;
pub mod handles;
pub mod middlewares;
pub mod models;
pub mod repositories;
pub mod services;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use tokio::net::TcpListener;

use crate::app::create_app;
use crate::configs::Settings;

pub async fn run(settings: &Arc<Settings>) {
    let app = create_app(settings).await;

    let ip_addr = settings.server.host.parse::<IpAddr>().unwrap();

    let address = SocketAddr::from((ip_addr, settings.server.port));

    let listener = TcpListener::bind(&address).await.unwrap();

    tracing::info!("listening on {:?}", address);

    axum::serve(listener, app).await.unwrap();
}

#[cfg(any(test, feature = "mock"))]
pub mod tests {
    use std::sync::Arc;

    use lumisync_api::{DeviceType, UserRole};
    use serde_json::{json, Value};
    use time::OffsetDateTime;

    use crate::configs::{Database, SchemaManager, Storage};
    use crate::models::*;

    pub async fn setup_test_db() -> Arc<Storage> {
        Arc::new(
            Storage::new(
                Database {
                    migration_path: None,
                    clean_start: true,
                    url: String::from("sqlite::memory:"),
                },
                SchemaManager::default(),
            )
            .await
            .unwrap(),
        )
    }

    pub async fn create_test_user(
        storage: Arc<Storage>,
        email: &str,
        password: &str,
        role: &UserRole,
    ) -> User {
        sqlx::query_as(
            r#"
            INSERT INTO users (email, password, role)
            VALUES ($1, $2, $3)
            RETURNING *;
            "#,
        )
        .bind(email)
        .bind(password)
        .bind(role.to_string())
        .fetch_one(storage.get_pool())
        .await
        .unwrap()
    }

    pub async fn create_test_group(storage: Arc<Storage>, name: &str) -> Group {
        sqlx::query_as(
            r#"
            INSERT INTO groups (name)
            VALUES ($1)
            RETURNING *;
            "#,
        )
        .bind(name)
        .fetch_one(storage.get_pool())
        .await
        .unwrap()
    }

    pub async fn create_test_user_group(
        storage: Arc<Storage>,
        user_id: i32,
        group_id: i32,
        active: bool,
    ) -> UserGroup {
        sqlx::query_as(
            r#"
            INSERT INTO users_groups_link (user_id, group_id, is_active)
            VALUES ($1, $2, $3)
            RETURNING *;
            "#,
        )
        .bind(user_id)
        .bind(group_id)
        .bind(active)
        .fetch_one(storage.get_pool())
        .await
        .unwrap()
    }

    pub async fn create_test_region(
        storage: Arc<Storage>,
        group_id: i32,
        name: &str,
        light: i32,
        temperature: f32,
        humidity: f32,
        is_public: bool,
    ) -> Region {
        sqlx::query_as(
            r#"
            INSERT INTO regions (group_id, name, light, temperature, humidity, is_public)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *;
            "#,
        )
        .bind(group_id)
        .bind(name)
        .bind(light)
        .bind(temperature)
        .bind(humidity)
        .bind(is_public)
        .fetch_one(storage.get_pool())
        .await
        .unwrap()
    }

    pub async fn create_test_region_setting(
        storage: Arc<Storage>,
        region_id: i32,
        min_light: i32,
        max_light: i32,
        min_temperature: f32,
        max_temperature: f32,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> RegionSetting {
        sqlx::query_as(
            r#"
            INSERT INTO regions_settings (region_id, min_light, max_light, min_temperature, max_temperature, start, end)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *;
            "#,
        )
        .bind(region_id)
        .bind(min_light)
        .bind(max_light)
        .bind(min_temperature)
        .bind(max_temperature)
        .bind(start)
        .bind(end)
        .fetch_one(storage.get_pool())
        .await
        .unwrap()
    }

    pub async fn create_test_user_region(
        storage: Arc<Storage>,
        user_id: i32,
        region_id: i32,
        role: &str,
    ) -> UserRegion {
        sqlx::query_as(
            r#"
            INSERT INTO users_regions_link (user_id, region_id, role)
            VALUES ($1, $2, $3)
            RETURNING *;
            "#,
        )
        .bind(user_id)
        .bind(region_id)
        .bind(role)
        .fetch_one(storage.get_pool())
        .await
        .unwrap()
    }

    pub async fn create_test_device(
        storage: Arc<Storage>,
        region_id: i32,
        name: &str,
        device_type: &DeviceType,
        status: Value,
    ) -> Device {
        sqlx::query_as(
            r#"
            INSERT INTO devices (region_id, name, device_type, location, status)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *;
            "#,
        )
        .bind(region_id)
        .bind(name)
        .bind(device_type.to_string())
        .bind(json!({"x": 0, "y": 0}))
        .bind(status)
        .fetch_one(storage.get_pool())
        .await
        .unwrap()
    }

    pub async fn create_test_device_setting(
        storage: Arc<Storage>,
        device_id: i32,
        setting_data: Value,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> DeviceSetting {
        sqlx::query_as(
            r#"
            INSERT INTO devices_settings (device_id, setting, start, end)
            VALUES ($1, $2, $3, $4)
            RETURNING *;
            "#,
        )
        .bind(device_id)
        .bind(&setting_data)
        .bind(start)
        .bind(end)
        .fetch_one(storage.get_pool())
        .await
        .unwrap()
    }

    pub async fn create_test_device_record(
        storage: Arc<Storage>,
        device_id: i32,
        data: Value,
        time: OffsetDateTime,
    ) -> DeviceRecord {
        sqlx::query_as(
            r#"
            INSERT INTO device_records (device_id, data, time)
            VALUES ($1, $2, $3)
            RETURNING *;
            "#,
        )
        .bind(device_id)
        .bind(&data)
        .bind(time)
        .fetch_one(storage.get_pool())
        .await
        .unwrap()
    }
}
