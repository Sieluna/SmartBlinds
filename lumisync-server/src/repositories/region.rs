use std::sync::Arc;

use sqlx::{Error, Sqlite, Transaction};

use crate::configs::Storage;
use crate::models::Region;

pub struct RegionRepository {
    storage: Arc<Storage>,
}

impl RegionRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }
}

impl RegionRepository {
    pub async fn create(
        &self,
        item: &Region,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO regions (group_id, name, light, temperature, humidity)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(item.group_id)
        .bind(&item.name)
        .bind(item.light)
        .bind(item.temperature)
        .bind(item.humidity)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<Region>, Error> {
        let region: Option<Region> = sqlx::query_as("SELECT * FROM regions WHERE id = $1")
            .bind(id)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(region)
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<Region>, Error> {
        let region: Option<Region> = sqlx::query_as("SELECT * FROM regions WHERE name = $1")
            .bind(name)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(region)
    }

    pub async fn find_by_group_id(&self, group_id: i32) -> Result<Vec<Region>, Error> {
        let regions: Vec<Region> = sqlx::query_as("SELECT * FROM regions WHERE group_id = $1")
            .bind(group_id)
            .fetch_all(self.storage.get_pool())
            .await?;

        Ok(regions)
    }

    pub async fn find_by_user_id(&self, user_id: i32) -> Result<Vec<Region>, Error> {
        let regions: Vec<Region> = sqlx::query_as(
            r#"
            SELECT r.* FROM regions r
            INNER JOIN users_regions_link url ON r.id = url.region_id
            WHERE url.user_id = $1
            UNION
            SELECT r.* FROM regions r
            INNER JOIN groups g ON r.group_id = g.id
            INNER JOIN users_groups_link ugl ON g.id = ugl.group_id
            WHERE ugl.user_id = $1 AND ugl.is_active = TRUE
            "#,
        )
        .bind(user_id)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(regions)
    }

    pub async fn update(
        &self,
        id: i32,
        item: &Region,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE regions
            SET name = $1, group_id = $2, light = $3, temperature = $4, humidity = $5
            WHERE id = $6
            "#,
        )
        .bind(&item.name)
        .bind(item.group_id)
        .bind(item.light)
        .bind(item.temperature)
        .bind(item.humidity)
        .bind(id)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    pub async fn update_environment_data(
        &self,
        id: i32,
        light: i32,
        temperature: f32,
        humidity: f32,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE regions
            SET light = $1, temperature = $2, humidity = $3
            WHERE id = $4
            "#,
        )
        .bind(light)
        .bind(temperature)
        .bind(humidity)
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
        sqlx::query("DELETE FROM regions WHERE id = $1")
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
    use crate::models::Group;
    use crate::repositories::GroupRepository;

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

    async fn create_test_group(storage: Arc<Storage>) -> i32 {
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

        group_id
    }

    #[tokio::test]
    async fn test_find_region_by_id() {
        let storage = setup_test_db().await;
        let group_id = create_test_group(storage.clone()).await;

        let region = Region {
            id: 0,
            group_id,
            name: "Living Room".to_string(),
            light: 500,
            temperature: 22.5,
            humidity: 45.0,
        };

        let repo = RegionRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&region, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_region = found.unwrap();
        assert_eq!(found_region.name, "Living Room");
        assert_eq!(found_region.light, 500);
        assert_eq!(found_region.temperature, 22.5);
    }

    #[tokio::test]
    async fn test_update_environment_data() {
        let storage = setup_test_db().await;
        let group_id = create_test_group(storage.clone()).await;

        let region = Region {
            id: 0,
            group_id,
            name: "Bedroom".to_string(),
            light: 300,
            temperature: 20.0,
            humidity: 40.0,
        };

        let repo = RegionRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&region, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update_environment_data(id, 350, 21.5, 42.5, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_region = found.unwrap();
        assert_eq!(found_region.light, 350);
        assert_eq!(found_region.temperature, 21.5);
        assert_eq!(found_region.humidity, 42.5);
    }

    #[tokio::test]
    async fn test_delete_region() {
        let storage = setup_test_db().await;
        let group_id = create_test_group(storage.clone()).await;

        let region = Region {
            id: 0,
            group_id,
            name: "Kitchen".to_string(),
            light: 600,
            temperature: 23.0,
            humidity: 50.0,
        };

        let repo = RegionRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&region, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_none());
    }
}
