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

#[cfg(test)]
mod tests {
    use lumisync_api::DeviceType;
    use serde_json::json;
    use time::OffsetDateTime;

    use crate::tests::*;

    use super::*;

    #[tokio::test]
    async fn test_create_and_find_device_record() {
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

        let now = OffsetDateTime::now_utc();
        let record = DeviceRecord {
            id: 0,
            device_id: device.id,
            data: json!({"temperature": 22.5, "humidity": 45.0}),
            time: now,
        };

        let repo = DeviceRecordRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&record, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());

        let found_record = found.unwrap();
        assert_eq!(found_record.device_id, device.id);
        if let Some(data) = found_record.data.as_object() {
            assert_eq!(data.get("temperature").unwrap().as_f64().unwrap(), 22.5);
            assert_eq!(data.get("humidity").unwrap().as_f64().unwrap(), 45.0);
        } else {
            panic!("Record data is not an object");
        }
    }

    #[tokio::test]
    async fn test_find_by_device_id() {
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
            &DeviceType::Sensor,
            json!({"online": true}),
        )
        .await;

        let now = OffsetDateTime::now_utc();
        create_test_device_record(
            storage.clone(),
            device1.id,
            json!({"temperature": 22.5, "humidity": 45.0}),
            now,
        )
        .await;
        create_test_device_record(
            storage.clone(),
            device1.id,
            json!({"temperature": 23.0, "humidity": 46.0}),
            now + time::Duration::minutes(5),
        )
        .await;
        create_test_device_record(
            storage.clone(),
            device2.id,
            json!({"temperature": 21.0, "humidity": 40.0}),
            now,
        )
        .await;

        let repo = DeviceRecordRepository::new(storage.clone());

        let records = repo.find_by_device_id(device1.id).await.unwrap();
        assert_eq!(records.len(), 2);

        let records = repo.find_by_device_id(device2.id).await.unwrap();
        assert_eq!(records.len(), 1);
    }

    #[tokio::test]
    async fn test_find_by_device_id_with_timerange() {
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

        let now = OffsetDateTime::now_utc();
        let time1 = now - time::Duration::hours(2);
        let time2 = now - time::Duration::hours(1);
        let time3 = now;
        let time4 = now + time::Duration::hours(1);

        create_test_device_record(
            storage.clone(),
            device.id,
            json!({"temperature": 20.0}),
            time1,
        )
        .await;
        create_test_device_record(
            storage.clone(),
            device.id,
            json!({"temperature": 21.0}),
            time2,
        )
        .await;
        create_test_device_record(
            storage.clone(),
            device.id,
            json!({"temperature": 22.0}),
            time3,
        )
        .await;
        create_test_device_record(
            storage.clone(),
            device.id,
            json!({"temperature": 23.0}),
            time4,
        )
        .await;

        let repo = DeviceRecordRepository::new(storage.clone());

        let records = repo
            .find_by_device_id_with_timerange(device.id, time2, time3)
            .await
            .unwrap();
        assert_eq!(records.len(), 2);

        assert!(records[0].time >= records[1].time);
    }

    #[tokio::test]
    async fn test_delete_device_record() {
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

        let now = OffsetDateTime::now_utc();
        let record = create_test_device_record(
            storage.clone(),
            device.id,
            json!({"temperature": 22.5, "humidity": 45.0}),
            now,
        )
        .await;

        let repo = DeviceRecordRepository::new(storage.clone());

        let found = repo.find_by_id(record.id).await.unwrap();
        assert!(found.is_some());

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(record.id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(record.id).await.unwrap();
        assert!(found.is_none());
    }
}
