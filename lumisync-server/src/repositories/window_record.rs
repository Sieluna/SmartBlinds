use std::sync::Arc;

use sqlx::{Error, Sqlite, Transaction};
use time::OffsetDateTime;

use crate::configs::Storage;
use crate::models::WindowRecord;

pub struct WindowRecordRepository {
    storage: Arc<Storage>,
}

impl WindowRecordRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }
}

impl WindowRecordRepository {
    // Create new window/blind record
    pub async fn create(
        &self,
        item: &WindowRecord,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO window_records (window_id, state, time)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(item.window_id)
        .bind(item.state)
        .bind(item.time)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    // Find window record by ID
    pub async fn find_by_id(&self, id: i32) -> Result<Option<WindowRecord>, Error> {
        let record: Option<WindowRecord> =
            sqlx::query_as("SELECT * FROM window_records WHERE id = $1")
                .bind(id)
                .fetch_optional(self.storage.get_pool())
                .await?;

        Ok(record)
    }

    // Get latest N records for a given window
    pub async fn find_latest_by_window_id(
        &self,
        window_id: i32,
        limit: i64,
    ) -> Result<Vec<WindowRecord>, Error> {
        let records: Vec<WindowRecord> = sqlx::query_as(
            r#"
            SELECT * FROM window_records
            WHERE window_id = $1
            ORDER BY time DESC
            LIMIT $2
            "#,
        )
        .bind(window_id)
        .bind(limit)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(records)
    }

    // Get window records within a given time range
    pub async fn find_by_window_id_and_time_range(
        &self,
        window_id: i32,
        start_time: OffsetDateTime,
        end_time: OffsetDateTime,
    ) -> Result<Vec<WindowRecord>, Error> {
        let records: Vec<WindowRecord> = sqlx::query_as(
            r#"
            SELECT * FROM window_records
            WHERE window_id = $1 AND time >= $2 AND time <= $3
            ORDER BY time ASC
            "#,
        )
        .bind(window_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(records)
    }

    // Get all window records in a region within a given time range
    pub async fn find_by_region_id_and_time_range(
        &self,
        region_id: i32,
        start_time: OffsetDateTime,
        end_time: OffsetDateTime,
    ) -> Result<Vec<WindowRecord>, Error> {
        let records: Vec<WindowRecord> = sqlx::query_as(
            r#"
            SELECT wr.* FROM window_records wr
            INNER JOIN windows w ON wr.window_id = w.id
            WHERE w.region_id = $1 AND wr.time >= $2 AND wr.time <= $3
            ORDER BY wr.time ASC
            "#,
        )
        .bind(region_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(records)
    }

    // Delete window record
    pub async fn delete(
        &self,
        id: i32,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query("DELETE FROM window_records WHERE id = $1")
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
        let result = sqlx::query("DELETE FROM window_records WHERE time < $1")
            .bind(time)
            .execute(&mut **transaction)
            .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use time::OffsetDateTime;

    use crate::configs::{Database, SchemaManager};
    use crate::models::{Group, Region, Window};
    use crate::repositories::{GroupRepository, RegionRepository, WindowRepository};

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

    async fn create_test_window(storage: Arc<Storage>) -> i32 {
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

        let window = Window {
            id: 0,
            region_id,
            name: "Test Window".to_string(),
            location: json!({"x": 10, "y": 20}),
            state: 0.0,
        };

        let window_repo = WindowRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let window_id = window_repo.create(&window, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        window_id
    }

    #[tokio::test]
    async fn test_create_and_find_window_record() {
        let storage = setup_test_db().await;
        let window_id = create_test_window(storage.clone()).await;

        let now = OffsetDateTime::now_utc();
        let record = WindowRecord {
            id: 0,
            window_id,
            state: 0.7,
            time: now,
        };

        let repo = WindowRecordRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&record, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_record = found.unwrap();
        assert_eq!(found_record.window_id, window_id);
        assert_eq!(found_record.state, 0.7);
    }

    #[tokio::test]
    async fn test_find_latest_records() {
        let storage = setup_test_db().await;
        let window_id = create_test_window(storage.clone()).await;

        let base_time = OffsetDateTime::now_utc();
        let records = vec![
            WindowRecord {
                id: 0,
                window_id,
                state: 0.0,
                time: base_time,
            },
            WindowRecord {
                id: 0,
                window_id,
                state: 0.5,
                time: base_time + time::Duration::minutes(5),
            },
            WindowRecord {
                id: 0,
                window_id,
                state: 1.0,
                time: base_time + time::Duration::minutes(10),
            },
        ];

        let repo = WindowRecordRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        for record in &records {
            repo.create(record, &mut tx).await.unwrap();
        }
        tx.commit().await.unwrap();

        let latest_records = repo.find_latest_by_window_id(window_id, 2).await.unwrap();
        assert_eq!(latest_records.len(), 2);
        assert_eq!(latest_records[0].state, 1.0);
        assert_eq!(latest_records[1].state, 0.5);
    }

    #[tokio::test]
    async fn test_find_by_time_range() {
        let storage = setup_test_db().await;
        let window_id = create_test_window(storage.clone()).await;

        let base_time = OffsetDateTime::now_utc();
        let records = vec![
            WindowRecord {
                id: 0,
                window_id,
                state: 0.0,
                time: base_time,
            },
            WindowRecord {
                id: 0,
                window_id,
                state: 0.5,
                time: base_time + time::Duration::minutes(5),
            },
            WindowRecord {
                id: 0,
                window_id,
                state: 1.0,
                time: base_time + time::Duration::minutes(10),
            },
        ];

        let repo = WindowRecordRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        for record in &records {
            repo.create(record, &mut tx).await.unwrap();
        }
        tx.commit().await.unwrap();

        let range_records = repo
            .find_by_window_id_and_time_range(
                window_id,
                base_time + time::Duration::minutes(3),
                base_time + time::Duration::minutes(7),
            )
            .await
            .unwrap();

        assert_eq!(range_records.len(), 1);
        assert_eq!(range_records[0].state, 0.5);
    }

    #[tokio::test]
    async fn test_delete_window_record() {
        let storage = setup_test_db().await;
        let window_id = create_test_window(storage.clone()).await;

        let now = OffsetDateTime::now_utc();
        let record = WindowRecord {
            id: 0,
            window_id,
            state: -0.5,
            time: now,
        };

        let repo = WindowRecordRepository::new(storage.clone());
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
