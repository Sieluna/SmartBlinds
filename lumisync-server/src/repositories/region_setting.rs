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

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use crate::tests::*;

    use super::*;

    #[tokio::test]
    async fn test_create_and_find_region_setting() {
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

        let now = OffsetDateTime::now_utc();
        let one_day_later = now + time::Duration::days(1);

        let setting = create_test_region_setting(
            storage.clone(),
            region.id,
            300,
            800,
            18.0,
            28.0,
            now,
            one_day_later,
        )
        .await;

        let repo = RegionSettingRepository::new(storage.clone());
        let found = repo.find_by_id(setting.id).await.unwrap();
        assert!(found.is_some());

        let found_setting = found.unwrap();
        assert_eq!(found_setting.region_id, setting.region_id);
        assert_eq!(found_setting.min_light, setting.min_light);
        assert_eq!(found_setting.max_light, setting.max_light);
        assert_eq!(found_setting.min_temperature, setting.min_temperature);
        assert_eq!(found_setting.max_temperature, setting.max_temperature);
    }

    #[tokio::test]
    async fn test_find_by_region_id() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let region1 = create_test_region(
            storage.clone(),
            group.id,
            "test_region_1",
            500,
            22.5,
            45.0,
            false,
        )
        .await;
        let region2 = create_test_region(
            storage.clone(),
            group.id,
            "test_region_2",
            600,
            23.0,
            50.0,
            false,
        )
        .await;

        let now = OffsetDateTime::now_utc();
        let one_day_later = now + time::Duration::days(1);
        let two_days_later = now + time::Duration::days(2);

        create_test_region_setting(
            storage.clone(),
            region1.id,
            300,
            800,
            18.0,
            28.0,
            now,
            one_day_later,
        )
        .await;

        create_test_region_setting(
            storage.clone(),
            region1.id,
            350,
            850,
            19.0,
            29.0,
            one_day_later,
            two_days_later,
        )
        .await;

        create_test_region_setting(
            storage.clone(),
            region2.id,
            400,
            900,
            20.0,
            30.0,
            now,
            two_days_later,
        )
        .await;

        let repo = RegionSettingRepository::new(storage.clone());

        let settings = repo.find_by_region_id(region1.id).await.unwrap();
        assert_eq!(settings.len(), 2);

        let settings = repo.find_by_region_id(region2.id).await.unwrap();
        assert_eq!(settings.len(), 1);
        assert_eq!(settings[0].region_id, region2.id);
    }

    #[tokio::test]
    async fn test_find_active_by_region_id() {
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

        let now = OffsetDateTime::now_utc();
        let one_hour_ago = now - time::Duration::hours(1);
        let one_hour_later = now + time::Duration::hours(1);
        let two_hours_later = now + time::Duration::hours(2);
        let three_hours_later = now + time::Duration::hours(3);

        create_test_region_setting(
            storage.clone(),
            region.id,
            100,
            600,
            15.0,
            25.0,
            one_hour_ago - time::Duration::hours(2),
            one_hour_ago,
        )
        .await;

        create_test_region_setting(
            storage.clone(),
            region.id,
            300,
            800,
            18.0,
            28.0,
            one_hour_ago,
            one_hour_later,
        )
        .await;

        create_test_region_setting(
            storage.clone(),
            region.id,
            350,
            850,
            19.0,
            29.0,
            one_hour_later,
            two_hours_later,
        )
        .await;

        create_test_region_setting(
            storage.clone(),
            region.id,
            400,
            900,
            20.0,
            30.0,
            two_hours_later,
            three_hours_later,
        )
        .await;

        let repo = RegionSettingRepository::new(storage.clone());
        let active = repo.find_active_by_region_id(region.id).await.unwrap();
        assert!(active.is_some());

        let active_setting = active.unwrap();
        assert_eq!(active_setting.min_light, 300);
        assert_eq!(active_setting.max_light, 800);
        assert_eq!(active_setting.min_temperature, 18.0);
        assert_eq!(active_setting.max_temperature, 28.0);
    }

    #[tokio::test]
    async fn test_update_region_setting() {
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

        let now = OffsetDateTime::now_utc();
        let one_day_later = now + time::Duration::days(1);

        let setting = create_test_region_setting(
            storage.clone(),
            region.id,
            300,
            800,
            18.0,
            28.0,
            now,
            one_day_later,
        )
        .await;

        let repo = RegionSettingRepository::new(storage.clone());

        let updated_setting = RegionSetting {
            id: setting.id,
            region_id: region.id,
            min_light: 350,
            max_light: 850,
            min_temperature: 19.0,
            max_temperature: 29.0,
            start: now,
            end: one_day_later + time::Duration::hours(12),
        };

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update(setting.id, &updated_setting, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(setting.id).await.unwrap();
        assert!(found.is_some());

        let found_setting = found.unwrap();
        assert_eq!(found_setting.min_light, 350);
        assert_eq!(found_setting.max_light, 850);
        assert_eq!(found_setting.min_temperature, 19.0);
        assert_eq!(found_setting.max_temperature, 29.0);
        assert!(found_setting.end > one_day_later);
    }

    #[tokio::test]
    async fn test_delete_region_setting() {
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

        let now = OffsetDateTime::now_utc();
        let one_day_later = now + time::Duration::days(1);

        let setting = create_test_region_setting(
            storage.clone(),
            region.id,
            300,
            800,
            18.0,
            28.0,
            now,
            one_day_later,
        )
        .await;

        let repo = RegionSettingRepository::new(storage.clone());

        let found = repo.find_by_id(setting.id).await.unwrap();
        assert!(found.is_some());

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(setting.id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(setting.id).await.unwrap();
        assert!(found.is_none());
    }
}
