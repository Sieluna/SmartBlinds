mod device;
mod device_record;
mod device_setting;
mod event;
mod group;
mod region;
mod region_setting;
mod user;
mod user_region;

pub use device::DeviceRepository;
pub use device_record::DeviceRecordRepository;
pub use device_setting::DeviceSettingRepository;
pub use event::EventRepository;
pub use group::GroupRepository;
pub use region::RegionRepository;
pub use region_setting::RegionSettingRepository;
pub use user::UserRepository;
pub use user_region::UserRegionRepository;

#[cfg(test)]
pub mod tests {
    use std::sync::Arc;

    use serde_json::Value;

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
        is_admin: bool,
    ) -> User {
        let role = if is_admin { "admin" } else { "user" };

        sqlx::query_as("INSERT INTO users (email, password, role) VALUES ($1, $2, $3) RETURNING *;")
            .bind(email)
            .bind(password)
            .bind(role)
            .fetch_one(storage.get_pool())
            .await
            .unwrap()
    }

    pub async fn create_test_group(storage: Arc<Storage>, name: &str) -> Group {
        sqlx::query_as("INSERT INTO groups (name) VALUES ($1) RETURNING *;")
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
        device_type: i32,
        status: Value,
    ) -> Device {
        use serde_json::json;

        sqlx::query_as(
            r#"
            INSERT INTO devices (region_id, name, device_type, location, status)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *;
            "#,
        )
        .bind(region_id)
        .bind(name)
        .bind(device_type)
        .bind(json!({"x": 0, "y": 0}))
        .bind(status)
        .fetch_one(storage.get_pool())
        .await
        .unwrap()
    }
}
