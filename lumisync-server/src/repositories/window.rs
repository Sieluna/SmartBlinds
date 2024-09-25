use std::sync::Arc;

use sqlx::{Error, Sqlite, Transaction};

use crate::configs::Storage;
use crate::models::Window;

pub struct WindowRepository {
    storage: Arc<Storage>,
}

impl WindowRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }
}

impl WindowRepository {
    pub async fn create(
        &self,
        item: &Window,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO windows (region_id, name, location, state)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(item.region_id)
        .bind(&item.name)
        .bind(&item.location)
        .bind(item.state)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<Window>, Error> {
        let window: Option<Window> = sqlx::query_as("SELECT * FROM windows WHERE id = $1")
            .bind(id)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(window)
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<Window>, Error> {
        let window: Option<Window> = sqlx::query_as("SELECT * FROM windows WHERE name = $1")
            .bind(name)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(window)
    }

    pub async fn find_by_region_id(&self, region_id: i32) -> Result<Vec<Window>, Error> {
        let windows: Vec<Window> = sqlx::query_as("SELECT * FROM windows WHERE region_id = $1")
            .bind(region_id)
            .fetch_all(self.storage.get_pool())
            .await?;

        Ok(windows)
    }

    pub async fn update(
        &self,
        id: i32,
        item: &Window,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE windows
            SET region_id = $1, name = $2, location = $3, state = $4
            WHERE id = $5
            "#,
        )
        .bind(item.region_id)
        .bind(&item.name)
        .bind(&item.location)
        .bind(item.state)
        .bind(id)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    pub async fn update_state(
        &self,
        id: i32,
        state: f32,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE windows
            SET state = $1
            WHERE id = $2
            "#,
        )
        .bind(state)
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
        sqlx::query("DELETE FROM windows WHERE id = $1")
            .bind(id)
            .execute(&mut **transaction)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
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
    async fn test_find_window_by_id() {
        let storage = setup_test_db().await;
        let region_id = create_test_region(storage.clone()).await;

        let window = Window {
            id: 0,
            region_id,
            name: "Living Room Window".to_string(),
            location: json!({"x": 10, "y": 20}),
            state: 0.0,
        };

        let repo = WindowRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&window, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_window = found.unwrap();
        assert_eq!(found_window.name, "Living Room Window");
        assert_eq!(found_window.state, 0.0);

        if let Some(location) = found_window.location.as_object() {
            assert_eq!(location.get("x").unwrap().as_i64().unwrap(), 10);
            assert_eq!(location.get("y").unwrap().as_i64().unwrap(), 20);
        } else {
            panic!("Location is not an object");
        }
    }

    #[tokio::test]
    async fn test_find_windows_by_region() {
        let storage = setup_test_db().await;
        let region_id = create_test_region(storage.clone()).await;

        let windows = vec![
            Window {
                id: 0,
                region_id,
                name: "Window 1".to_string(),
                location: json!({"x": 10, "y": 20}),
                state: 0.0,
            },
            Window {
                id: 0,
                region_id,
                name: "Window 2".to_string(),
                location: json!({"x": 30, "y": 40}),
                state: 0.5,
            },
        ];

        let repo = WindowRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        for window in &windows {
            repo.create(window, &mut tx).await.unwrap();
        }
        tx.commit().await.unwrap();

        let found_windows = repo.find_by_region_id(region_id).await.unwrap();
        assert_eq!(found_windows.len(), 2);

        let window_names: Vec<String> = found_windows.iter().map(|w| w.name.clone()).collect();
        assert!(window_names.contains(&"Window 1".to_string()));
        assert!(window_names.contains(&"Window 2".to_string()));
    }

    #[tokio::test]
    async fn test_update_window_state() {
        let storage = setup_test_db().await;
        let region_id = create_test_region(storage.clone()).await;

        let window = Window {
            id: 0,
            region_id,
            name: "Adjustable Window".to_string(),
            location: json!({"x": 50, "y": 60}),
            state: 0.0,
        };

        let repo = WindowRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&window, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update_state(id, 0.5, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_window = found.unwrap();
        assert_eq!(found_window.state, 0.5);

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update_state(id, 1.0, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_window = found.unwrap();
        assert_eq!(found_window.state, 1.0);
    }

    #[tokio::test]
    async fn test_delete_window() {
        let storage = setup_test_db().await;
        let region_id = create_test_region(storage.clone()).await;

        let window = Window {
            id: 0,
            region_id,
            name: "Window to Delete".to_string(),
            location: json!({"x": 70, "y": 80}),
            state: 0.0,
        };

        let repo = WindowRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&window, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_none());
    }
}
