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
    use crate::repositories::tests::*;

    use super::*;

    #[tokio::test]
    async fn test_find_sensor_by_id() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region =
            create_test_region(storage.clone(), group.id, "test_region", 500, 22.0, 45.0).await;
        let sensor = create_test_sensor(storage.clone(), region.id, "test_sensor").await;

        let repo = SensorRepository::new(storage.clone());
        let found = repo.find_by_id(sensor.id).await.unwrap();
        assert!(found.is_some());

        let found_sensor = found.unwrap();
        assert_eq!(found_sensor.name, sensor.name);
        assert_eq!(found_sensor.region_id, region.id);
    }

    #[tokio::test]
    async fn test_find_sensor_by_name() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region =
            create_test_region(storage.clone(), group.id, "test_region", 500, 22.0, 45.0).await;
        let sensor1 = create_test_sensor(storage.clone(), region.id, "test_sensor_1").await;
        let sensor2 = create_test_sensor(storage.clone(), region.id, "test_sensor_2").await;

        let repo = SensorRepository::new(storage.clone());
        let found_sensor1 = repo.find_by_name(&sensor1.name).await.unwrap();
        assert!(found_sensor1.is_some());

        let found_sensor1 = found_sensor1.unwrap();
        assert_eq!(found_sensor1.name, sensor1.name);
        assert_eq!(found_sensor1.region_id, region.id);

        let found_sensor2 = repo.find_by_name(&sensor2.name).await.unwrap();
        assert!(found_sensor2.is_some());

        let found_sensor2 = found_sensor2.unwrap();
        assert_eq!(found_sensor2.name, sensor2.name);
        assert_eq!(found_sensor2.region_id, region.id);

        let not_found = repo.find_by_name("non_existent_sensor").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_find_sensors_by_region() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region =
            create_test_region(storage.clone(), group.id, "test_region", 500, 22.0, 45.0).await;
        let sensor1 = create_test_sensor(storage.clone(), region.id, "test_sensor_1").await;
        let sensor2 = create_test_sensor(storage.clone(), region.id, "test_sensor_2").await;

        let repo = SensorRepository::new(storage.clone());
        let found_sensors = repo.find_by_region_id(region.id).await.unwrap();
        assert_eq!(found_sensors.len(), 2);

        let sensor_names: Vec<String> = found_sensors.iter().map(|s| s.name.clone()).collect();
        assert!(sensor_names.contains(&sensor1.name));
        assert!(sensor_names.contains(&sensor2.name));
    }

    #[tokio::test]
    async fn test_update_sensor() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region =
            create_test_region(storage.clone(), group.id, "test_region", 500, 22.0, 45.0).await;
        let sensor = create_test_sensor(storage.clone(), region.id, "test_sensor").await;

        let repo = SensorRepository::new(storage.clone());
        let updated = Sensor {
            id: sensor.id,
            region_id: sensor.region_id,
            name: "updated_sensor".to_string(),
        };

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update(sensor.id, &updated, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(sensor.id).await.unwrap();
        assert!(found.is_some());

        let found_sensor = found.unwrap();
        assert_eq!(found_sensor.name, updated.name);
    }

    #[tokio::test]
    async fn test_delete_sensor() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region =
            create_test_region(storage.clone(), group.id, "test_region", 500, 22.0, 45.0).await;
        let sensor = create_test_sensor(storage.clone(), region.id, "test_sensor").await;

        let repo = SensorRepository::new(storage.clone());

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(sensor.id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(sensor.id).await.unwrap();
        assert!(found.is_none());
    }
}
