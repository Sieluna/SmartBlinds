use std::sync::Arc;

use sqlx::{Error, Sqlite, Transaction};

use crate::configs::Storage;
use crate::models::Sensor;

pub struct SensorRepository {
    storage: Arc<Storage>,
}

impl SensorRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }
}

impl SensorRepository {
    // Create new sensor
    pub async fn create(
        &self,
        item: &Sensor,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO sensors (region_id, name)
            VALUES ($1, $2)
            "#,
        )
        .bind(item.region_id)
        .bind(&item.name)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    // Find sensor by ID
    pub async fn find_by_id(&self, id: i32) -> Result<Option<Sensor>, Error> {
        let sensor: Option<Sensor> = sqlx::query_as("SELECT * FROM sensors WHERE id = $1")
            .bind(id)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(sensor)
    }

    // Find sensor by name
    pub async fn find_by_name(&self, name: &str) -> Result<Option<Sensor>, Error> {
        let sensor: Option<Sensor> = sqlx::query_as("SELECT * FROM sensors WHERE name = $1")
            .bind(name)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(sensor)
    }

    // Get all sensors in a region
    pub async fn find_by_region_id(&self, region_id: i32) -> Result<Vec<Sensor>, Error> {
        let sensors: Vec<Sensor> = sqlx::query_as("SELECT * FROM sensors WHERE region_id = $1")
            .bind(region_id)
            .fetch_all(self.storage.get_pool())
            .await?;

        Ok(sensors)
    }

    // Update sensor information
    pub async fn update(
        &self,
        id: i32,
        item: &Sensor,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE sensors
            SET region_id = $1, name = $2
            WHERE id = $3
            "#,
        )
        .bind(item.region_id)
        .bind(&item.name)
        .bind(id)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    // Delete sensor
    pub async fn delete(
        &self,
        id: i32,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query("DELETE FROM sensors WHERE id = $1")
            .bind(id)
            .execute(&mut **transaction)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use crate::configs::{Database, SchemaManager};
    use crate::models::{Group, Region};
    use crate::repositories::{GroupRepository, RegionRepository};

    use super::*;

    async fn setup_test_db() -> Arc<Storage> {
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

    async fn create_test_region(storage: Arc<Storage>) -> i32 {
        let now = OffsetDateTime::now_utc();
        let group = Group {
            id: 0,
            name: "Test Group".to_string(),
            description: Some("A test group".to_string()),
            created_at: now,
        };

        let group_repo = GroupRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let group_id = group_repo.create(&group, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let region = Region {
            id: 0,
            group_id,
            name: "Test Region".to_string(),
            light: 500,
            temperature: 22.0,
            humidity: 45.0,
        };

        let region_repo = RegionRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let region_id = region_repo.create(&region, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        region_id
    }

    #[tokio::test]
    async fn test_find_sensor_by_id() {
        let storage = setup_test_db().await;
        let region_id = create_test_region(storage.clone()).await;

        let sensor = Sensor {
            id: 0,
            region_id,
            name: "Temperature Sensor 1".to_string(),
        };

        let repo = SensorRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&sensor, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_sensor = found.unwrap();
        assert_eq!(found_sensor.name, "Temperature Sensor 1");
        assert_eq!(found_sensor.region_id, region_id);
    }

    #[tokio::test]
    async fn test_find_sensors_by_region() {
        let storage = setup_test_db().await;
        let region_id = create_test_region(storage.clone()).await;

        let sensors = vec![
            Sensor {
                id: 0,
                region_id,
                name: "Light Sensor 1".to_string(),
            },
            Sensor {
                id: 0,
                region_id,
                name: "Humidity Sensor 1".to_string(),
            },
        ];

        let repo = SensorRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        for sensor in &sensors {
            repo.create(sensor, &mut tx).await.unwrap();
        }
        tx.commit().await.unwrap();

        let found_sensors = repo.find_by_region_id(region_id).await.unwrap();
        assert_eq!(found_sensors.len(), 2);

        let sensor_names: Vec<String> = found_sensors.iter().map(|s| s.name.clone()).collect();
        assert!(sensor_names.contains(&"Light Sensor 1".to_string()));
        assert!(sensor_names.contains(&"Humidity Sensor 1".to_string()));
    }

    #[tokio::test]
    async fn test_update_sensor() {
        let storage = setup_test_db().await;
        let region_id = create_test_region(storage.clone()).await;

        let sensor = Sensor {
            id: 0,
            region_id,
            name: "Original Sensor".to_string(),
        };

        let repo = SensorRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&sensor, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let updated_sensor = Sensor {
            id,
            region_id,
            name: "Updated Sensor".to_string(),
        };

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update(id, &updated_sensor, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_sensor = found.unwrap();
        assert_eq!(found_sensor.name, "Updated Sensor");
    }

    #[tokio::test]
    async fn test_delete_sensor() {
        let storage = setup_test_db().await;
        let region_id = create_test_region(storage.clone()).await;

        let sensor = Sensor {
            id: 0,
            region_id,
            name: "Sensor to Delete".to_string(),
        };

        let repo = SensorRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&sensor, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_none());
    }
}
