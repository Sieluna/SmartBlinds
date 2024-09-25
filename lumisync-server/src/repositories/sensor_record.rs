use std::sync::Arc;

use sqlx::{Error, Sqlite, Transaction};
use time::OffsetDateTime;

use crate::configs::Storage;
use crate::models::SensorRecord;

pub struct SensorRecordRepository {
    storage: Arc<Storage>,
}

impl SensorRecordRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }
}

impl SensorRecordRepository {
    // Create new sensor record
    pub async fn create(
        &self,
        item: &SensorRecord,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO sensor_records (sensor_id, light, temperature, humidity, time)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(item.sensor_id)
        .bind(item.light)
        .bind(item.temperature)
        .bind(item.humidity)
        .bind(item.time)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    // Find sensor record by ID
    pub async fn find_by_id(&self, id: i32) -> Result<Option<SensorRecord>, Error> {
        let record: Option<SensorRecord> =
            sqlx::query_as("SELECT * FROM sensor_records WHERE id = $1")
                .bind(id)
                .fetch_optional(self.storage.get_pool())
                .await?;

        Ok(record)
    }

    // Get latest N records for a given sensor
    pub async fn find_latest_by_sensor_id(
        &self,
        sensor_id: i32,
        limit: i64,
    ) -> Result<Vec<SensorRecord>, Error> {
        let records: Vec<SensorRecord> = sqlx::query_as(
            r#"
            SELECT * FROM sensor_records
            WHERE sensor_id = $1
            ORDER BY time DESC
            LIMIT $2
            "#,
        )
        .bind(sensor_id)
        .bind(limit)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(records)
    }

    // Get sensor records within a given time range
    pub async fn find_by_sensor_id_and_time_range(
        &self,
        sensor_id: i32,
        start_time: OffsetDateTime,
        end_time: OffsetDateTime,
    ) -> Result<Vec<SensorRecord>, Error> {
        let records: Vec<SensorRecord> = sqlx::query_as(
            r#"
            SELECT * FROM sensor_records
            WHERE sensor_id = $1 AND time >= $2 AND time <= $3
            ORDER BY time ASC
            "#,
        )
        .bind(sensor_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(records)
    }

    // Get all sensor records in a region within a given time range
    pub async fn find_by_region_id_and_time_range(
        &self,
        region_id: i32,
        start_time: OffsetDateTime,
        end_time: OffsetDateTime,
    ) -> Result<Vec<SensorRecord>, Error> {
        let records: Vec<SensorRecord> = sqlx::query_as(
            r#"
            SELECT sr.* FROM sensor_records sr
            INNER JOIN sensors s ON sr.sensor_id = s.id
            WHERE s.region_id = $1 AND sr.time >= $2 AND sr.time <= $3
            ORDER BY sr.time ASC
            "#,
        )
        .bind(region_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(records)
    }

    // Delete sensor record
    pub async fn delete(
        &self,
        id: i32,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query("DELETE FROM sensor_records WHERE id = $1")
            .bind(id)
            .execute(&mut **transaction)
            .await?;

        Ok(())
    }

    // Delete all records before a given time (data cleanup)
    pub async fn delete_before_time(
        &self,
        time: OffsetDateTime,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<u64, Error> {
        let result = sqlx::query("DELETE FROM sensor_records WHERE time < $1")
            .bind(time)
            .execute(&mut **transaction)
            .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use crate::configs::{Database, SchemaManager};
    use crate::models::{Group, Region, Sensor};
    use crate::repositories::{GroupRepository, RegionRepository, SensorRepository};

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

    // Create test sensor, return sensor ID
    async fn create_test_sensor(storage: Arc<Storage>) -> i32 {
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

        let sensor = Sensor {
            id: 0,
            region_id,
            name: "Test Sensor".to_string(),
        };

        let sensor_repo = SensorRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let sensor_id = sensor_repo.create(&sensor, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        sensor_id
    }

    #[tokio::test]
    async fn test_create_and_find_sensor_record() {
        let storage = setup_test_db().await;
        let sensor_id = create_test_sensor(storage.clone()).await;

        let now = OffsetDateTime::now_utc();
        let record = SensorRecord {
            id: 0,
            sensor_id,
            light: 500,
            temperature: 22.5,
            humidity: 45.0,
            time: now,
        };

        let repo = SensorRecordRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&record, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_record = found.unwrap();
        assert_eq!(found_record.sensor_id, sensor_id);
        assert_eq!(found_record.light, 500);
        assert_eq!(found_record.temperature, 22.5);
        assert_eq!(found_record.humidity, 45.0);
    }

    #[tokio::test]
    async fn test_find_latest_records() {
        let storage = setup_test_db().await;
        let sensor_id = create_test_sensor(storage.clone()).await;

        let base_time = OffsetDateTime::now_utc();
        let records = vec![
            SensorRecord {
                id: 0,
                sensor_id,
                light: 100,
                temperature: 20.0,
                humidity: 40.0,
                time: base_time,
            },
            SensorRecord {
                id: 0,
                sensor_id,
                light: 150,
                temperature: 21.0,
                humidity: 41.0,
                time: base_time + time::Duration::minutes(5),
            },
            SensorRecord {
                id: 0,
                sensor_id,
                light: 200,
                temperature: 22.0,
                humidity: 42.0,
                time: base_time + time::Duration::minutes(10),
            },
        ];

        let repo = SensorRecordRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        for record in &records {
            repo.create(record, &mut tx).await.unwrap();
        }
        tx.commit().await.unwrap();

        let latest_records = repo.find_latest_by_sensor_id(sensor_id, 2).await.unwrap();
        assert_eq!(latest_records.len(), 2);
        assert_eq!(latest_records[0].light, 200);
        assert_eq!(latest_records[1].light, 150);
    }

    #[tokio::test]
    async fn test_find_by_time_range() {
        let storage = setup_test_db().await;
        let sensor_id = create_test_sensor(storage.clone()).await;

        let base_time = OffsetDateTime::now_utc();
        let records = vec![
            SensorRecord {
                id: 0,
                sensor_id,
                light: 100,
                temperature: 20.0,
                humidity: 40.0,
                time: base_time,
            },
            SensorRecord {
                id: 0,
                sensor_id,
                light: 150,
                temperature: 21.0,
                humidity: 41.0,
                time: base_time + time::Duration::minutes(5),
            },
            SensorRecord {
                id: 0,
                sensor_id,
                light: 200,
                temperature: 22.0,
                humidity: 42.0,
                time: base_time + time::Duration::minutes(10),
            },
        ];

        let repo = SensorRecordRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        for record in &records {
            repo.create(record, &mut tx).await.unwrap();
        }
        tx.commit().await.unwrap();

        let range_records = repo
            .find_by_sensor_id_and_time_range(
                sensor_id,
                base_time + time::Duration::minutes(3),
                base_time + time::Duration::minutes(7),
            )
            .await
            .unwrap();

        assert_eq!(range_records.len(), 1);
        assert_eq!(range_records[0].light, 150);
    }

    #[tokio::test]
    async fn test_delete_sensor_record() {
        let storage = setup_test_db().await;
        let sensor_id = create_test_sensor(storage.clone()).await;

        let now = OffsetDateTime::now_utc();
        let record = SensorRecord {
            id: 0,
            sensor_id,
            light: 300,
            temperature: 25.0,
            humidity: 50.0,
            time: now,
        };

        let repo = SensorRecordRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&record, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_none());
    }
}
