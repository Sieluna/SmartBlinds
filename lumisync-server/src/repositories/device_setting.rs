use std::sync::Arc;

use sqlx::{Error, Pool, Sqlite, Transaction};
use time::OffsetDateTime;

use crate::configs::Storage;
use crate::models::DeviceSetting;

#[derive(Clone)]
pub struct DeviceSettingRepository {
    storage: Arc<Storage>,
}

impl DeviceSettingRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub fn get_pool(&self) -> &Pool<Sqlite> {
        self.storage.get_pool()
    }
}

impl DeviceSettingRepository {
    pub async fn create(
        &self,
        item: &DeviceSetting,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO devices_settings (device_id, setting, start, end)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(item.device_id)
        .bind(&item.setting)
        .bind(item.start)
        .bind(item.end)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<DeviceSetting>, Error> {
        let setting: Option<DeviceSetting> =
            sqlx::query_as("SELECT * FROM devices_settings WHERE id = $1")
                .bind(id)
                .fetch_optional(self.storage.get_pool())
                .await?;

        Ok(setting)
    }

    pub async fn find_by_device_id(&self, device_id: i32) -> Result<Vec<DeviceSetting>, Error> {
        let settings: Vec<DeviceSetting> =
            sqlx::query_as("SELECT * FROM devices_settings WHERE device_id = $1")
                .bind(device_id)
                .fetch_all(self.storage.get_pool())
                .await?;

        Ok(settings)
    }

    pub async fn find_active_by_device_id(
        &self,
        device_id: i32,
    ) -> Result<Vec<DeviceSetting>, Error> {
        let now = OffsetDateTime::now_utc();
        let settings: Vec<DeviceSetting> = sqlx::query_as(
            r#"
            SELECT * FROM devices_settings 
            WHERE device_id = $1 AND start <= $2 AND end >= $2
            "#,
        )
        .bind(device_id)
        .bind(now)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(settings)
    }

    pub async fn update(
        &self,
        id: i32,
        item: &DeviceSetting,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE devices_settings
            SET device_id = $1, setting = $2, start = $3, end = $4
            WHERE id = $5
            "#,
        )
        .bind(item.device_id)
        .bind(&item.setting)
        .bind(item.start)
        .bind(item.end)
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
        sqlx::query("DELETE FROM devices_settings WHERE id = $1")
            .bind(id)
            .execute(&mut **transaction)
            .await?;

        Ok(())
    }
}
