use std::collections::HashMap;
use std::sync::Arc;

use lumisync_api::RegionRole;
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
