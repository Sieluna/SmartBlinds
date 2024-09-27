use std::sync::Arc;

use sqlx::{Error, Pool, Sqlite, Transaction};
use time::OffsetDateTime;

use crate::configs::Storage;
use crate::models::RegionSetting;

#[derive(Clone)]
pub struct RegionSettingRepository {
    storage: Arc<Storage>,
}

impl RegionSettingRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub fn get_pool(&self) -> &Pool<Sqlite> {
        self.storage.get_pool()
    }
}

impl RegionSettingRepository {
    pub async fn create(
        &self,
        item: &RegionSetting,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO regions_settings (region_id, min_light, max_light, min_temperature, max_temperature, start, end)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(item.region_id)
        .bind(item.min_light)
        .bind(item.max_light)
        .bind(item.min_temperature)
        .bind(item.max_temperature)
        .bind(item.start)
        .bind(item.end)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<RegionSetting>, Error> {
        let setting: Option<RegionSetting> =
            sqlx::query_as("SELECT * FROM regions_settings WHERE id = $1")
                .bind(id)
                .fetch_optional(self.storage.get_pool())
                .await?;

        Ok(setting)
    }

    pub async fn find_by_region_id(&self, region_id: i32) -> Result<Vec<RegionSetting>, Error> {
        let settings: Vec<RegionSetting> =
            sqlx::query_as("SELECT * FROM regions_settings WHERE region_id = $1")
                .bind(region_id)
                .fetch_all(self.storage.get_pool())
                .await?;

        Ok(settings)
    }

    pub async fn find_active_by_region_id(
        &self,
        region_id: i32,
    ) -> Result<Option<RegionSetting>, Error> {
        let now = OffsetDateTime::now_utc();
        let setting: Option<RegionSetting> = sqlx::query_as(
            r#"
            SELECT * FROM regions_settings 
            WHERE region_id = $1 AND start <= $2 AND end >= $2
            ORDER BY id DESC
            LIMIT 1
            "#,
        )
        .bind(region_id)
        .bind(now)
        .fetch_optional(self.storage.get_pool())
        .await?;

        Ok(setting)
    }

    pub async fn update(
        &self,
        id: i32,
        item: &RegionSetting,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE regions_settings
            SET region_id = $1, min_light = $2, max_light = $3, min_temperature = $4, max_temperature = $5, start = $6, end = $7
            WHERE id = $8
            "#,
        )
        .bind(item.region_id)
        .bind(item.min_light)
        .bind(item.max_light)
        .bind(item.min_temperature)
        .bind(item.max_temperature)
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
        sqlx::query("DELETE FROM regions_settings WHERE id = $1")
            .bind(id)
            .execute(&mut **transaction)
            .await?;

        Ok(())
    }
}
