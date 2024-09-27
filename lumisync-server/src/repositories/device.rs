use std::sync::Arc;

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
        .bind(item.device_type)
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

    pub async fn find_by_type(&self, device_type: i32) -> Result<Vec<Device>, Error> {
        let devices: Vec<Device> = sqlx::query_as("SELECT * FROM devices WHERE device_type = $1")
            .bind(device_type)
            .fetch_all(self.storage.get_pool())
            .await?;

        Ok(devices)
    }

    pub async fn find_by_region_and_type(
        &self,
        region_id: i32,
        device_type: i32,
    ) -> Result<Vec<Device>, Error> {
        let devices: Vec<Device> =
            sqlx::query_as("SELECT * FROM devices WHERE region_id = $1 AND device_type = $2")
                .bind(region_id)
                .bind(device_type)
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
        .bind(item.device_type)
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
