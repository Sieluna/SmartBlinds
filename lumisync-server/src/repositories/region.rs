use std::sync::Arc;

use sqlx::{Error, Pool, Sqlite, Transaction};

use crate::configs::Storage;
use crate::models::Region;

#[derive(Clone)]
pub struct RegionRepository {
    storage: Arc<Storage>,
}

impl RegionRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub fn get_pool(&self) -> &Pool<Sqlite> {
        self.storage.get_pool()
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
            INSERT INTO regions (group_id, name, light, temperature, humidity, is_public)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(item.group_id)
        .bind(&item.name)
        .bind(item.light)
        .bind(item.temperature)
        .bind(item.humidity)
        .bind(item.is_public)
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
            SET name = $1, group_id = $2, light = $3, temperature = $4, humidity = $5, is_public = $6
            WHERE id = $7
            "#,
        )
        .bind(&item.name)
        .bind(item.group_id)
        .bind(item.light)
        .bind(item.temperature)
        .bind(item.humidity)
        .bind(item.is_public)
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

    pub async fn update_visibility(
        &self,
        id: i32,
        is_public: bool,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE regions
            SET is_public = $1
            WHERE id = $2
            "#,
        )
        .bind(is_public)
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
    use lumisync_api::models::UserRole;

    use crate::tests::*;

    use super::*;

    #[tokio::test]
    async fn test_find_region_by_id() {
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

        let repo = RegionRepository::new(storage.clone());
        let found = repo.find_by_id(region.id).await.unwrap();
        assert!(found.is_some());

        let found_region = found.unwrap();
        assert_eq!(found_region.name, region.name);
        assert_eq!(found_region.light, region.light);
        assert_eq!(found_region.temperature, region.temperature);
        assert_eq!(found_region.humidity, region.humidity);
    }

    #[tokio::test]
    async fn test_find_region_by_name() {
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

        let repo = RegionRepository::new(storage.clone());
        let found = repo.find_by_name(&region.name).await.unwrap();
        assert!(found.is_some());

        let found_region = found.unwrap();
        assert_eq!(found_region.group_id, group.id);
        assert_eq!(found_region.name, region.name);
        assert_eq!(found_region.light, region.light);
        assert_eq!(found_region.temperature, region.temperature);
        assert_eq!(found_region.humidity, region.humidity);
    }

    #[tokio::test]
    async fn test_find_regions_by_group_id() {
        let storage = setup_test_db().await;
        let group1 = create_test_group(storage.clone(), "test_group_1").await;
        let group2 = create_test_group(storage.clone(), "test_group_2").await;
        let region1 = create_test_region(
            storage.clone(),
            group1.id,
            "test_region_1",
            400,
            21.0,
            40.0,
            false,
        )
        .await;
        let region2 = create_test_region(
            storage.clone(),
            group1.id,
            "test_region_2",
            450,
            22.0,
            45.0,
            false,
        )
        .await;
        let region3 = create_test_region(
            storage.clone(),
            group2.id,
            "test_region_3",
            500,
            23.0,
            50.0,
            false,
        )
        .await;

        let repo = RegionRepository::new(storage.clone());

        let found_group1_regions = repo.find_by_group_id(group1.id).await.unwrap();
        assert_eq!(found_group1_regions.len(), 2);

        let region_names: Vec<String> = found_group1_regions
            .iter()
            .map(|r| r.name.clone())
            .collect();
        assert!(region_names.contains(&region1.name));
        assert!(region_names.contains(&region2.name));
        assert!(!region_names.contains(&region3.name));

        let found_group2_regions = repo.find_by_group_id(group2.id).await.unwrap();
        assert_eq!(found_group2_regions.len(), 1);
        assert_eq!(found_group2_regions[0].name, region3.name);

        let non_existent_group_regions = repo.find_by_group_id(9999).await.unwrap();
        assert!(non_existent_group_regions.is_empty());
    }

    #[tokio::test]
    async fn test_find_regions_by_user_id() {
        let storage = setup_test_db().await;
        let user =
            create_test_user(storage.clone(), "test@test.com", "test", &UserRole::User).await;
        let group1 = create_test_group(storage.clone(), "test_group_1").await;
        let group2 = create_test_group(storage.clone(), "test_group_2").await;
        let group3 = create_test_group(storage.clone(), "test_group_3").await;
        create_test_user_group(storage.clone(), user.id, group1.id, true).await;
        create_test_user_group(storage.clone(), user.id, group2.id, false).await;
        let region1 = create_test_region(
            storage.clone(),
            group1.id,
            "test_region_1",
            400,
            21.0,
            40.0,
            false,
        )
        .await;
        let region2 = create_test_region(
            storage.clone(),
            group2.id,
            "test_region_2",
            450,
            22.0,
            45.0,
            false,
        )
        .await;
        let region3 = create_test_region(
            storage.clone(),
            group3.id,
            "test_region_3",
            500,
            23.0,
            50.0,
            false,
        )
        .await;
        create_test_user_region(storage.clone(), user.id, region3.id, "admin").await;

        let repo = RegionRepository::new(storage.clone());
        let found_regions = repo.find_by_user_id(user.id).await.unwrap();

        assert_eq!(found_regions.len(), 2);

        let region_names: Vec<String> = found_regions.iter().map(|r| r.name.clone()).collect();
        assert!(region_names.contains(&region1.name));
        assert!(!region_names.contains(&region2.name));
        assert!(region_names.contains(&region3.name));

        let non_existent_user_regions = repo.find_by_user_id(9999).await.unwrap();
        assert!(non_existent_user_regions.is_empty());
    }

    #[tokio::test]
    async fn test_update_region() {
        let storage = setup_test_db().await;
        let group1 = create_test_group(storage.clone(), "test_group_1").await;
        let group2 = create_test_group(storage.clone(), "test_group_2").await;
        let region = create_test_region(
            storage.clone(),
            group1.id,
            "test_region",
            500,
            22.5,
            45.0,
            false,
        )
        .await;

        let repo = RegionRepository::new(storage.clone());

        let updated_region = Region {
            id: region.id,
            group_id: group2.id,
            name: "updated_region".to_string(),
            light: 600,
            temperature: 23.5,
            humidity: 50.0,
            is_public: true,
        };

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update(region.id, &updated_region, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(region.id).await.unwrap();
        assert!(found.is_some());

        let found_region = found.unwrap();
        assert_eq!(found_region.name, "updated_region");
        assert_eq!(found_region.group_id, group2.id);
        assert_eq!(found_region.light, 600);
        assert_eq!(found_region.temperature, 23.5);
        assert_eq!(found_region.humidity, 50.0);
        assert!(found_region.is_public);
    }

    #[tokio::test]
    async fn test_update_environment_data() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region = create_test_region(
            storage.clone(),
            group.id,
            "test_region",
            300,
            20.0,
            40.0,
            false,
        )
        .await;

        let repo = RegionRepository::new(storage.clone());

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update_environment_data(region.id, 350, 21.5, 42.5, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(region.id).await.unwrap();
        assert!(found.is_some());
        let found_region = found.unwrap();
        assert_eq!(found_region.light, 350);
        assert_eq!(found_region.temperature, 21.5);
        assert_eq!(found_region.humidity, 42.5);
    }

    #[tokio::test]
    async fn test_update_visibility() {
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

        let repo = RegionRepository::new(storage.clone());

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update_visibility(region.id, true, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(region.id).await.unwrap();
        assert!(found.is_some());
        let found_region = found.unwrap();
        assert!(found_region.is_public);

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update_visibility(region.id, false, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(region.id).await.unwrap();
        assert!(found.is_some());
        let found_region = found.unwrap();
        assert!(!found_region.is_public);
    }

    #[tokio::test]
    async fn test_delete_region() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region = create_test_region(
            storage.clone(),
            group.id,
            "test_region",
            600,
            23.0,
            50.0,
            false,
        )
        .await;

        let repo = RegionRepository::new(storage.clone());

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(region.id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(region.id).await.unwrap();
        assert!(found.is_none());
    }
}
