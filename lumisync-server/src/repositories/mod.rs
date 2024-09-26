mod device;
mod event;
mod group;
mod region;
mod user;

pub use device::DeviceRepository;
pub use event::EventRepository;
pub use group::GroupRepository;
pub use region::RegionRepository;
pub use user::UserRepository;

#[cfg(test)]
pub mod tests {
    use std::sync::Arc;

    use lumisync_api::restful::Role;

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

    pub async fn create_test_user(storage: Arc<Storage>, email: &str, password: &str) -> User {
        sqlx::query_as("INSERT INTO users (email, password) VALUES ($1, $2) RETURNING *;")
            .bind(email)
            .bind(password)
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
        role: Role,
        active: bool,
    ) -> UserGroup {
        sqlx::query_as(
            r#"
            INSERT INTO users_groups_link (user_id, group_id, role, is_active)
            VALUES ($1, $2, $3, $4)
            RETURNING *;
            "#,
        )
        .bind(user_id)
        .bind(group_id)
        .bind(role.to_string())
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
    ) -> Region {
        sqlx::query_as(
            r#"
            INSERT INTO regions (group_id, name, light, temperature, humidity)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *;
            "#,
        )
        .bind(group_id)
        .bind(name)
        .bind(light)
        .bind(temperature)
        .bind(humidity)
        .fetch_one(storage.get_pool())
        .await
        .unwrap()
    }

    pub async fn create_test_user_region(
        storage: Arc<Storage>,
        user_id: i32,
        region_id: i32,
    ) -> UserRegion {
        sqlx::query_as(
            r#"
            INSERT INTO users_regions_link (user_id, region_id)
            VALUES ($1, $2)
            RETURNING *;
            "#,
        )
        .bind(user_id)
        .bind(region_id)
        .fetch_one(storage.get_pool())
        .await
        .unwrap()
    }

    pub async fn create_test_device(
        storage: Arc<Storage>, 
        region_id: i32, 
        name: &str,
        device_type: &str
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
        .bind(0.0)
        .fetch_one(storage.get_pool())
        .await
        .unwrap()
    }
}
