use std::sync::Arc;

use lumisync_api::DeviceType;
use serde_json::Value;
use sqlx::{Error, Pool, Sqlite, Transaction};

use crate::configs::Storage;
use crate::models::Device;

#[derive(Clone)]
pub struct DeviceRepository {
    storage: Arc<Storage>,
}

impl DeviceRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub fn get_pool(&self) -> &Pool<Sqlite> {
        self.storage.get_pool()
    }
}

impl DeviceRepository {
    pub async fn create(
        &self,
        item: &Device,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO devices (region_id, name, device_type, location, status)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(item.region_id)
        .bind(&item.name)
        .bind(&item.device_type)
        .bind(&item.location)
        .bind(&item.status)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<Device>, Error> {
        let device: Option<Device> = sqlx::query_as("SELECT * FROM devices WHERE id = $1")
            .bind(id)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(device)
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<Device>, Error> {
        let device: Option<Device> = sqlx::query_as("SELECT * FROM devices WHERE name = $1")
            .bind(name)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(device)
    }

    pub async fn find_by_region_id(&self, region_id: i32) -> Result<Vec<Device>, Error> {
        let devices: Vec<Device> = sqlx::query_as("SELECT * FROM devices WHERE region_id = $1")
            .bind(region_id)
            .fetch_all(self.storage.get_pool())
            .await?;

        Ok(devices)
    }

    pub async fn find_by_type(&self, device_type: &DeviceType) -> Result<Vec<Device>, Error> {
        let devices: Vec<Device> = sqlx::query_as("SELECT * FROM devices WHERE device_type = $1")
            .bind(device_type.to_string())
            .fetch_all(self.storage.get_pool())
            .await?;

        Ok(devices)
    }

    pub async fn find_by_region_and_type(
        &self,
        region_id: i32,
        device_type: &DeviceType,
    ) -> Result<Vec<Device>, Error> {
        let devices: Vec<Device> =
            sqlx::query_as("SELECT * FROM devices WHERE region_id = $1 AND device_type = $2")
                .bind(region_id)
                .bind(device_type.to_string())
                .fetch_all(self.storage.get_pool())
                .await?;

        Ok(devices)
    }

    pub async fn update(
        &self,
        id: i32,
        item: &Device,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE devices
            SET region_id = $1, name = $2, device_type = $3, location = $4, status = $5
            WHERE id = $6
            "#,
        )
        .bind(item.region_id)
        .bind(&item.name)
        .bind(&item.device_type)
        .bind(&item.location)
        .bind(&item.status)
        .bind(id)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    pub async fn update_status(
        &self,
        id: i32,
        status: &Value,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE devices
            SET status = $1
            WHERE id = $2
            "#,
        )
        .bind(status)
        .bind(id)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    pub async fn delete(
        &self,
        id: i32,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query("DELETE FROM devices WHERE id = $1")
            .bind(id)
            .execute(&mut **transaction)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use lumisync_api::DeviceType;
    use serde_json::json;

    use crate::tests::*;

    use super::*;

    #[tokio::test]
    async fn test_find_device_by_id() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region = create_test_region(
            storage.clone(),
            group.id,
            "test_region",
            500,
            22.5,
            45.0,
            false,
        )
        .await;
        let device = create_test_device(
            storage.clone(),
            region.id,
            "test_device",
            &DeviceType::Sensor,
            json!({"online": true}),
        )
        .await;

        let repo = DeviceRepository::new(storage.clone());
        let found = repo.find_by_id(device.id).await.unwrap();
        assert!(found.is_some());

        let found_device = found.unwrap();
        assert_eq!(found_device.name, device.name);
        assert_eq!(found_device.device_type, device.device_type);
        assert_eq!(found_device.region_id, region.id);
    }

    #[tokio::test]
    async fn test_find_device_by_name() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region = create_test_region(
            storage.clone(),
            group.id,
            "test_region",
            500,
            22.5,
            45.0,
            false,
        )
        .await;

        let device = create_test_device(
            storage.clone(),
            region.id,
            "test_device",
            &DeviceType::Sensor,
            json!({"online": true}),
        )
        .await;

        let repo = DeviceRepository::new(storage.clone());
        let found = repo.find_by_name(&device.name).await.unwrap();
        assert!(found.is_some());

        let found_device = found.unwrap();
        assert_eq!(found_device.name, device.name);
    }

    #[tokio::test]
    async fn test_find_devices_by_region_id() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region1 = create_test_region(
            storage.clone(),
            group.id,
            "test_region_1",
            500,
            22.5,
            45.0,
            false,
        )
        .await;
        let region2 = create_test_region(
            storage.clone(),
            group.id,
            "test_region_2",
            400,
            20.0,
            40.0,
            false,
        )
        .await;
        let device1 = create_test_device(
            storage.clone(),
            region1.id,
            "test_device_1",
            &DeviceType::Sensor,
            json!({"online": true}),
        )
        .await;
        let device2 = create_test_device(
            storage.clone(),
            region1.id,
            "test_device_2",
            &DeviceType::Window,
            json!({"online": false}),
        )
        .await;
        let device3 = create_test_device(
            storage.clone(),
            region2.id,
            "test_device_3",
            &DeviceType::Sensor,
            json!({"online": true}),
        )
        .await;

        let repo = DeviceRepository::new(storage.clone());
        let devices = repo.find_by_region_id(region1.id).await.unwrap();
        assert_eq!(devices.len(), 2);

        let device_names: Vec<String> = devices.iter().map(|d| d.name.clone()).collect();
        assert!(device_names.contains(&device1.name));
        assert!(device_names.contains(&device2.name));
        assert!(!device_names.contains(&device3.name));
    }

    #[tokio::test]
    async fn test_find_devices_by_type() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region = create_test_region(
            storage.clone(),
            group.id,
            "test_region",
            500,
            22.5,
            45.0,
            false,
        )
        .await;
        let device1 = create_test_device(
            storage.clone(),
            region.id,
            "test_device_1",
            &DeviceType::Sensor,
            json!({"online": true}),
        )
        .await;
        let device2 = create_test_device(
            storage.clone(),
            region.id,
            "test_device_2",
            &DeviceType::Window,
            json!({"online": false}),
        )
        .await;

        let repo = DeviceRepository::new(storage.clone());
        let type1_devices = repo.find_by_type(&DeviceType::Sensor).await.unwrap();
        assert_eq!(type1_devices.len(), 1);
        assert_eq!(type1_devices[0].name, device1.name);

        let type2_devices = repo.find_by_type(&DeviceType::Window).await.unwrap();
        assert_eq!(type2_devices.len(), 1);
        assert_eq!(type2_devices[0].name, device2.name);
    }

    #[tokio::test]
    async fn test_find_devices_by_region_and_type() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region1 = create_test_region(
            storage.clone(),
            group.id,
            "test_region_1",
            500,
            22.5,
            45.0,
            false,
        )
        .await;
        let region2 = create_test_region(
            storage.clone(),
            group.id,
            "test_region_2",
            600,
            23.0,
            50.0,
            false,
        )
        .await;
        let device1 = create_test_device(
            storage.clone(),
            region1.id,
            "test_device_1",
            &DeviceType::Sensor,
            json!({"online": true}),
        )
        .await;
        let device2 = create_test_device(
            storage.clone(),
            region1.id,
            "test_device_2",
            &DeviceType::Window,
            json!({"online": false}),
        )
        .await;
        let device3 = create_test_device(
            storage.clone(),
            region2.id,
            "test_device_3",
            &DeviceType::Sensor,
            json!({"online": true}),
        )
        .await;

        let repo = DeviceRepository::new(storage.clone());

        let devices = repo
            .find_by_region_and_type(region1.id, &DeviceType::Sensor)
            .await
            .unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, device1.name);
        assert_eq!(devices[0].device_type, DeviceType::Sensor.to_string());

        let devices = repo
            .find_by_region_and_type(region1.id, &DeviceType::Window)
            .await
            .unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, device2.name);
        assert_eq!(devices[0].device_type, DeviceType::Window.to_string());

        let devices = repo
            .find_by_region_and_type(region2.id, &DeviceType::Sensor)
            .await
            .unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, device3.name);
        assert_eq!(devices[0].device_type, DeviceType::Sensor.to_string());

        let devices = repo
            .find_by_region_and_type(region2.id, &DeviceType::Window)
            .await
            .unwrap();
        assert_eq!(devices.len(), 0);
    }

    #[tokio::test]
    async fn test_update_device() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region = create_test_region(
            storage.clone(),
            group.id,
            "test_region",
            500,
            22.5,
            45.0,
            false,
        )
        .await;

        let device = create_test_device(
            storage.clone(),
            region.id,
            "test_device",
            &DeviceType::Sensor,
            json!({"online": true}),
        )
        .await;

        let repo = DeviceRepository::new(storage.clone());
        let updated_device = Device {
            id: device.id,
            region_id: region.id,
            name: "updated_device".to_string(),
            device_type: DeviceType::Window.to_string(),
            location: json!({"x": 15, "y": 25}),
            status: json!({"online": false}),
        };

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update(device.id, &updated_device, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(device.id).await.unwrap();
        assert!(found.is_some());

        let found_device = found.unwrap();
        assert_eq!(found_device.name, "updated_device");
        assert_eq!(found_device.device_type, DeviceType::Window.to_string());
    }

    #[tokio::test]
    async fn test_update_device_status() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region = create_test_region(
            storage.clone(),
            group.id,
            "test_region",
            500,
            22.5,
            45.0,
            false,
        )
        .await;

        let device = create_test_device(
            storage.clone(),
            region.id,
            "test_device",
            &DeviceType::Sensor,
            json!({"online": true}),
        )
        .await;

        let repo = DeviceRepository::new(storage.clone());
        let updated_status = json!({"online": false, "error": "connection lost"});
        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update_status(device.id, &updated_status, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(device.id).await.unwrap();
        assert!(found.is_some());

        let found_device = found.unwrap();
        assert_eq!(found_device.status, updated_status);
    }

    #[tokio::test]
    async fn test_delete_device() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region = create_test_region(
            storage.clone(),
            group.id,
            "test_region",
            500,
            22.5,
            45.0,
            false,
        )
        .await;

        let device = create_test_device(
            storage.clone(),
            region.id,
            "test_device",
            &DeviceType::Sensor,
            json!({"online": true}),
        )
        .await;

        let repo = DeviceRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(device.id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(device.id).await.unwrap();
        assert!(found.is_none());
    }
}
