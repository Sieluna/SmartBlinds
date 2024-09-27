use std::sync::Arc;

use sqlx::{Error, Pool, Sqlite, Transaction};
use time::OffsetDateTime;

use crate::configs::Storage;
use crate::models::DeviceRecord;

#[derive(Clone)]
pub struct DeviceRecordRepository {
    storage: Arc<Storage>,
}

impl DeviceRecordRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub fn get_pool(&self) -> &Pool<Sqlite> {
        self.storage.get_pool()
    }
}

impl DeviceRecordRepository {
    pub async fn create(
        &self,
        item: &DeviceRecord,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO device_records (device_id, data, time)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(item.device_id)
        .bind(&item.data)
        .bind(item.time)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<DeviceRecord>, Error> {
        let record: Option<DeviceRecord> =
            sqlx::query_as("SELECT * FROM device_records WHERE id = $1")
                .bind(id)
                .fetch_optional(self.storage.get_pool())
                .await?;

        Ok(record)
    }

    pub async fn find_by_device_id(&self, device_id: i32) -> Result<Vec<DeviceRecord>, Error> {
        let records: Vec<DeviceRecord> =
            sqlx::query_as("SELECT * FROM device_records WHERE device_id = $1")
                .bind(device_id)
                .fetch_all(self.storage.get_pool())
                .await?;

        Ok(records)
    }

    pub async fn find_by_device_id_with_timerange(
        &self,
        device_id: i32,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> Result<Vec<DeviceRecord>, Error> {
        let records: Vec<DeviceRecord> = sqlx::query_as(
            r#"
            SELECT * FROM device_records 
            WHERE device_id = $1 AND time >= $2 AND time <= $3
            ORDER BY time DESC
            "#,
        )
        .bind(device_id)
        .bind(start)
        .bind(end)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(records)
    }

    pub async fn delete(
        &self,
        id: i32,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query("DELETE FROM device_records WHERE id = $1")
            .bind(id)
            .execute(&mut **transaction)
            .await?;

        Ok(())
    }
}
