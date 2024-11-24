use std::collections::HashMap;
use std::sync::Arc;

use lumisync_api::models::RegionRole;
use sqlx::{Error, Pool, Sqlite, Transaction};

use crate::configs::Storage;
use crate::models::UserRegion;

#[derive(Clone)]
pub struct UserRegionRepository {
    storage: Arc<Storage>,
}

impl UserRegionRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub fn get_pool(&self) -> &Pool<Sqlite> {
        self.storage.get_pool()
    }
}

impl UserRegionRepository {
    pub async fn create(
        &self,
        item: &UserRegion,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO users_regions_link (user_id, region_id, role, is_active)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(item.user_id)
        .bind(item.region_id)
        .bind(&item.role)
        .bind(item.is_active)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<UserRegion>, Error> {
        let link: Option<UserRegion> =
            sqlx::query_as("SELECT * FROM users_regions_link WHERE id = $1")
                .bind(id)
                .fetch_optional(self.storage.get_pool())
                .await?;

        Ok(link)
    }

    pub async fn find_by_user_and_region(
        &self,
        user_id: i32,
        region_id: i32,
    ) -> Result<Option<UserRegion>, Error> {
        let link: Option<UserRegion> = sqlx::query_as(
            r#"
            SELECT * FROM users_regions_link 
            WHERE user_id = $1 AND region_id = $2
            "#,
        )
        .bind(user_id)
        .bind(region_id)
        .fetch_optional(self.storage.get_pool())
        .await?;

        Ok(link)
    }

    pub async fn get_region_roles_by_region_id(
        &self,
        region_id: i32,
    ) -> Result<HashMap<i32, RegionRole>, Error> {
        let links: Vec<UserRegion> = sqlx::query_as(
            r#"
            SELECT * FROM users_regions_link
            WHERE region_id = $1 AND is_active = TRUE
            "#,
        )
        .bind(region_id)
        .fetch_all(self.storage.get_pool())
        .await?;

        let mut roles = HashMap::new();
        for link in links {
            roles.insert(link.user_id, link.role.into());
        }

        Ok(roles)
    }

    pub async fn update_role(
        &self,
        user_id: i32,
        region_id: i32,
        role: &str,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE users_regions_link
            SET role = $1
            WHERE user_id = $2 AND region_id = $3
            "#,
        )
        .bind(role)
        .bind(user_id)
        .bind(region_id)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    pub async fn update_active_status(
        &self,
        user_id: i32,
        region_id: i32,
        is_active: bool,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE users_regions_link
            SET is_active = $1
            WHERE user_id = $2 AND region_id = $3
            "#,
        )
        .bind(is_active)
        .bind(user_id)
        .bind(region_id)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    pub async fn delete(
        &self,
        id: i32,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query("DELETE FROM users_regions_link WHERE id = $1")
            .bind(id)
            .execute(&mut **transaction)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use lumisync_api::models::{RegionRole, UserRole};

    use crate::tests::*;

    use super::*;

    #[tokio::test]
    async fn test_create_and_find_user_region() {
        let storage = setup_test_db().await;
        let user =
            create_test_user(storage.clone(), "test@test.com", "test", &UserRole::User).await;
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

        let user_region = UserRegion {
            id: 0,
            user_id: user.id,
            region_id: region.id,
            role: "owner".to_string(),
            joined_at: time::OffsetDateTime::now_utc(),
            is_active: true,
        };

        let repo = UserRegionRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&user_region, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());

        let found_link = found.unwrap();
        assert_eq!(found_link.user_id, user.id);
        assert_eq!(found_link.region_id, region.id);
        assert_eq!(found_link.role, "owner");
        assert!(found_link.is_active);
    }

    #[tokio::test]
    async fn test_find_by_user_and_region() {
        let storage = setup_test_db().await;
        let user1 =
            create_test_user(storage.clone(), "user1@test.com", "test", &UserRole::User).await;
        let user2 =
            create_test_user(storage.clone(), "user2@test.com", "test", &UserRole::User).await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region1 =
            create_test_region(storage.clone(), group.id, "region1", 500, 22.5, 45.0, false).await;
        let region2 =
            create_test_region(storage.clone(), group.id, "region2", 600, 23.0, 50.0, false).await;

        let user1_region1 =
            create_test_user_region(storage.clone(), user1.id, region1.id, "owner").await;
        let user1_region2 =
            create_test_user_region(storage.clone(), user1.id, region2.id, "visitor").await;
        let user2_region1 =
            create_test_user_region(storage.clone(), user2.id, region1.id, "visitor").await;

        let repo = UserRegionRepository::new(storage.clone());

        let link = repo
            .find_by_user_and_region(user1.id, region1.id)
            .await
            .unwrap();
        assert!(link.is_some());
        let found = link.unwrap();
        assert_eq!(found.id, user1_region1.id);
        assert_eq!(found.role, "owner");

        let link = repo
            .find_by_user_and_region(user1.id, region2.id)
            .await
            .unwrap();
        assert!(link.is_some());
        let found = link.unwrap();
        assert_eq!(found.id, user1_region2.id);
        assert_eq!(found.role, "visitor");

        let link = repo
            .find_by_user_and_region(user2.id, region1.id)
            .await
            .unwrap();
        assert!(link.is_some());
        let found = link.unwrap();
        assert_eq!(found.id, user2_region1.id);
        assert_eq!(found.role, "visitor");

        let link = repo
            .find_by_user_and_region(user2.id, region2.id)
            .await
            .unwrap();
        assert!(link.is_none());
    }

    #[tokio::test]
    async fn test_get_region_roles_by_region_id() {
        let storage = setup_test_db().await;
        let user1 =
            create_test_user(storage.clone(), "user1@test.com", "test", &UserRole::User).await;
        let user2 =
            create_test_user(storage.clone(), "user2@test.com", "test", &UserRole::User).await;
        let user3 =
            create_test_user(storage.clone(), "user3@test.com", "test", &UserRole::User).await;
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

        create_test_user_region(storage.clone(), user1.id, region.id, "owner").await;

        let mut tx = storage.get_pool().begin().await.unwrap();
        sqlx::query(
            "UPDATE users_regions_link SET is_active = $1 WHERE user_id = $2 AND region_id = $3",
        )
        .bind(true)
        .bind(user1.id)
        .bind(region.id)
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();

        create_test_user_region(storage.clone(), user2.id, region.id, "visitor").await;

        let mut tx = storage.get_pool().begin().await.unwrap();
        sqlx::query(
            "UPDATE users_regions_link SET is_active = $1 WHERE user_id = $2 AND region_id = $3",
        )
        .bind(true)
        .bind(user2.id)
        .bind(region.id)
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();

        create_test_user_region(storage.clone(), user3.id, region.id, "visitor").await;

        let mut tx = storage.get_pool().begin().await.unwrap();
        sqlx::query(
            "UPDATE users_regions_link SET is_active = $1 WHERE user_id = $2 AND region_id = $3",
        )
        .bind(false)
        .bind(user3.id)
        .bind(region.id)
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let repo = UserRegionRepository::new(storage.clone());
        let roles = repo.get_region_roles_by_region_id(region.id).await.unwrap();

        assert_eq!(roles.len(), 2);
        assert!(roles.contains_key(&user1.id));
        assert!(roles.contains_key(&user2.id));
        assert!(!roles.contains_key(&user3.id));

        assert_eq!(roles.get(&user1.id), Some(&RegionRole::Owner));
        assert_eq!(roles.get(&user2.id), Some(&RegionRole::Visitor));
    }

    #[tokio::test]
    async fn test_update_role() {
        let storage = setup_test_db().await;
        let user =
            create_test_user(storage.clone(), "test@test.com", "test", &UserRole::User).await;
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

        create_test_user_region(storage.clone(), user.id, region.id, "visitor").await;

        let repo = UserRegionRepository::new(storage.clone());

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update_role(user.id, region.id, "owner", &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let link = repo
            .find_by_user_and_region(user.id, region.id)
            .await
            .unwrap();
        assert!(link.is_some());
        let found = link.unwrap();
        assert_eq!(found.role, "owner");
    }

    #[tokio::test]
    async fn test_update_active_status() {
        let storage = setup_test_db().await;
        let user =
            create_test_user(storage.clone(), "test@test.com", "test", &UserRole::User).await;
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

        create_test_user_region(storage.clone(), user.id, region.id, "owner").await;

        let repo = UserRegionRepository::new(storage.clone());

        let link = repo
            .find_by_user_and_region(user.id, region.id)
            .await
            .unwrap();
        assert!(link.unwrap().is_active);

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update_active_status(user.id, region.id, false, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let link = repo
            .find_by_user_and_region(user.id, region.id)
            .await
            .unwrap();
        assert!(!link.unwrap().is_active);

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update_active_status(user.id, region.id, true, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let link = repo
            .find_by_user_and_region(user.id, region.id)
            .await
            .unwrap();
        assert!(link.unwrap().is_active);
    }

    #[tokio::test]
    async fn test_delete_user_region() {
        let storage = setup_test_db().await;
        let user =
            create_test_user(storage.clone(), "test@test.com", "test", &UserRole::User).await;
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

        let user_region =
            create_test_user_region(storage.clone(), user.id, region.id, "owner").await;

        let repo = UserRegionRepository::new(storage.clone());

        let found = repo.find_by_id(user_region.id).await.unwrap();
        assert!(found.is_some());

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(user_region.id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(user_region.id).await.unwrap();
        assert!(found.is_none());

        let link = repo
            .find_by_user_and_region(user.id, region.id)
            .await
            .unwrap();
        assert!(link.is_none());
    }
}
