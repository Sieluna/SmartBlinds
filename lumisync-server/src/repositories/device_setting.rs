use std::sync::Arc;

use sqlx::{Error, Pool, Sqlite, Transaction};
use time::OffsetDateTime;

use crate::configs::Storage;
use crate::models::DeviceSetting;

#[derive(Clone)]
pub struct DeviceSettingRepository {
    storage: Arc<Storage>,
}

impl DeviceSettingRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub fn get_pool(&self) -> &Pool<Sqlite> {
        self.storage.get_pool()
    }
}

impl DeviceSettingRepository {
    pub async fn create(
        &self,
        item: &DeviceSetting,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO devices_settings (device_id, setting, start, end)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(item.device_id)
        .bind(&item.setting)
        .bind(item.start)
        .bind(item.end)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<DeviceSetting>, Error> {
        let setting: Option<DeviceSetting> =
            sqlx::query_as("SELECT * FROM devices_settings WHERE id = $1")
                .bind(id)
                .fetch_optional(self.storage.get_pool())
                .await?;

        Ok(setting)
    }

    pub async fn find_by_device_id(&self, device_id: i32) -> Result<Vec<DeviceSetting>, Error> {
        let settings: Vec<DeviceSetting> =
            sqlx::query_as("SELECT * FROM devices_settings WHERE device_id = $1")
                .bind(device_id)
                .fetch_all(self.storage.get_pool())
                .await?;

        Ok(settings)
    }

    pub async fn find_active_by_device_id(
        &self,
        device_id: i32,
    ) -> Result<Vec<DeviceSetting>, Error> {
        let now = OffsetDateTime::now_utc();
        let settings: Vec<DeviceSetting> = sqlx::query_as(
            r#"
            SELECT * FROM devices_settings 
            WHERE device_id = $1 AND start <= $2 AND end >= $2
            "#,
        )
        .bind(device_id)
        .bind(now)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(settings)
    }

    pub async fn update(
        &self,
        id: i32,
        item: &DeviceSetting,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE devices_settings
            SET device_id = $1, setting = $2, start = $3, end = $4
            WHERE id = $5
            "#,
        )
        .bind(item.device_id)
        .bind(&item.setting)
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
        sqlx::query("DELETE FROM devices_settings WHERE id = $1")
            .bind(id)
            .execute(&mut **transaction)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use lumisync_api::models::DeviceType;
    use serde_json::json;
    use time::OffsetDateTime;

    use crate::tests::*;

    use super::*;

    #[tokio::test]
    async fn test_create_and_find_device_setting() {
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
            &DeviceType::Window,
            json!({"online": true}),
        )
        .await;

        let now = OffsetDateTime::now_utc();
        let one_day_later = now + time::Duration::days(1);
        let setting = DeviceSetting {
            id: 0,
            device_id: device.id,
            setting: json!({"position": 45, "brightness": 80}),
            start: now,
            end: one_day_later,
        };

        let repo = DeviceSettingRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&setting, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());

        let found_setting = found.unwrap();
        assert_eq!(found_setting.device_id, device.id);
        if let Some(data) = found_setting.setting.as_object() {
            assert_eq!(data.get("position").unwrap().as_i64().unwrap(), 45);
            assert_eq!(data.get("brightness").unwrap().as_i64().unwrap(), 80);
        } else {
            panic!("Setting data is not an object");
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
            &DeviceType::Window,
            json!({"online": true}),
        )
        .await;
        let device2 = create_test_device(
            storage.clone(),
            region.id,
            "test_device_2",
            &DeviceType::Window,
            json!({"online": true}),
        )
        .await;

        let now = OffsetDateTime::now_utc();
        let one_day_later = now + time::Duration::days(1);
        let two_days_later = now + time::Duration::days(2);

        create_test_device_setting(
            storage.clone(),
            device1.id,
            json!({"position": 45, "brightness": 80}),
            now,
            one_day_later,
        )
        .await;
        create_test_device_setting(
            storage.clone(),
            device1.id,
            json!({"position": 30, "brightness": 60}),
            one_day_later,
            two_days_later,
        )
        .await;
        create_test_device_setting(
            storage.clone(),
            device2.id,
            json!({"position": 20, "brightness": 50}),
            now,
            two_days_later,
        )
        .await;

        let repo = DeviceSettingRepository::new(storage.clone());

        let settings = repo.find_by_device_id(device1.id).await.unwrap();
        assert_eq!(settings.len(), 2);

        let settings = repo.find_by_device_id(device2.id).await.unwrap();
        assert_eq!(settings.len(), 1);
    }

    #[tokio::test]
    async fn test_find_active_by_device_id() {
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
            &DeviceType::Window,
            json!({"online": true}),
        )
        .await;

        let now = OffsetDateTime::now_utc();
        let past = now - time::Duration::hours(2);
        let future = now + time::Duration::hours(2);
        let far_future = now + time::Duration::hours(4);

        create_test_device_setting(
            storage.clone(),
            device.id,
            json!({"position": 10, "brightness": 30}),
            past - time::Duration::hours(2),
            past,
        )
        .await;

        create_test_device_setting(
            storage.clone(),
            device.id,
            json!({"position": 45, "brightness": 80}),
            past,
            future,
        )
        .await;

        create_test_device_setting(
            storage.clone(),
            device.id,
            json!({"position": 60, "brightness": 90}),
            future,
            far_future,
        )
        .await;

        let repo = DeviceSettingRepository::new(storage.clone());

        let active_settings = repo.find_active_by_device_id(device.id).await.unwrap();
        assert_eq!(active_settings.len(), 1);

        if let Some(data) = active_settings[0].setting.as_object() {
            assert_eq!(data.get("position").unwrap().as_i64().unwrap(), 45);
            assert_eq!(data.get("brightness").unwrap().as_i64().unwrap(), 80);
        } else {
            panic!("Setting data is not an object");
        }
    }

    #[tokio::test]
    async fn test_update_device_setting() {
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
            &DeviceType::Window,
            json!({"online": true}),
        )
        .await;

        let now = OffsetDateTime::now_utc();
        let one_day_later = now + time::Duration::days(1);
        let setting = create_test_device_setting(
            storage.clone(),
            device.id,
            json!({"position": 45, "brightness": 80}),
            now,
            one_day_later,
        )
        .await;

        let updated_setting = DeviceSetting {
            id: setting.id,
            device_id: device.id,
            setting: json!({"position": 50, "brightness": 85}),
            start: now,
            end: one_day_later + time::Duration::hours(12),
        };

        let repo = DeviceSettingRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update(setting.id, &updated_setting, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(setting.id).await.unwrap();
        assert!(found.is_some());

        let found_setting = found.unwrap();
        if let Some(data) = found_setting.setting.as_object() {
            assert_eq!(data.get("position").unwrap().as_i64().unwrap(), 50);
            assert_eq!(data.get("brightness").unwrap().as_i64().unwrap(), 85);
        } else {
            panic!("Setting data is not an object");
        }
        assert!(found_setting.end > one_day_later);
    }

    #[tokio::test]
    async fn test_delete_device_setting() {
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
            &DeviceType::Window,
            json!({"online": true}),
        )
        .await;

        let now = OffsetDateTime::now_utc();
        let one_day_later = now + time::Duration::days(1);
        let setting = create_test_device_setting(
            storage.clone(),
            device.id,
            json!({"position": 45, "brightness": 80}),
            now,
            one_day_later,
        )
        .await;

        let repo = DeviceSettingRepository::new(storage.clone());

        let found = repo.find_by_id(setting.id).await.unwrap();
        assert!(found.is_some());

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(setting.id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(setting.id).await.unwrap();
        assert!(found.is_none());
    }
}
