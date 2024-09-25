use std::sync::Arc;

use sqlx::{Error, Sqlite, Transaction};

use crate::configs::Storage;
use crate::models::Group;

pub struct GroupRepository {
    storage: Arc<Storage>,
}

impl GroupRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }
}

impl GroupRepository {
    pub async fn create(
        &self,
        item: &Group,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO groups (name, description, created_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(&item.name)
        .bind(&item.description)
        .bind(item.created_at)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<Group>, Error> {
        let group: Option<Group> = sqlx::query_as("SELECT * FROM groups WHERE id = $1")
            .bind(id)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(group)
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<Group>, Error> {
        let group: Option<Group> = sqlx::query_as("SELECT * FROM groups WHERE name = $1")
            .bind(name)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(group)
    }

    pub async fn find_all(&self) -> Result<Vec<Group>, Error> {
        let groups: Vec<Group> = sqlx::query_as("SELECT * FROM groups")
            .fetch_all(self.storage.get_pool())
            .await?;

        Ok(groups)
    }

    pub async fn find_by_user_id(&self, user_id: i32) -> Result<Vec<Group>, Error> {
        let groups: Vec<Group> = sqlx::query_as(
            r#"
            SELECT g.* FROM groups g
            INNER JOIN users_groups_link ugl ON g.id = ugl.group_id
            WHERE ugl.user_id = $1 AND ugl.is_active = TRUE
            "#,
        )
        .bind(user_id)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(groups)
    }

    pub async fn update(
        &self,
        id: i32,
        item: &Group,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE groups
            SET name = $1, description = $2
            WHERE id = $3
            "#,
        )
        .bind(&item.name)
        .bind(&item.description)
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
        sqlx::query("DELETE FROM groups WHERE id = $1")
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

    #[tokio::test]
    async fn test_find_group_by_id() {
        let storage = setup_test_db().await;

        let now = OffsetDateTime::now_utc();
        let group = Group {
            id: 0,
            name: "Test Group".to_string(),
            description: Some("A test group".to_string()),
            created_at: now,
        };

        let repo = GroupRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&group, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_group = found.unwrap();
        assert_eq!(found_group.name, "Test Group");
        assert_eq!(found_group.description, Some("A test group".to_string()));
    }

    #[tokio::test]
    async fn test_find_group_by_name() {
        let storage = setup_test_db().await;

        let now = OffsetDateTime::now_utc();
        let group = Group {
            id: 0,
            name: "Named Group".to_string(),
            description: None,
            created_at: now,
        };

        let repo = GroupRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.create(&group, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_name("Named Group").await.unwrap();
        assert!(found.is_some());
        let found_group = found.unwrap();
        assert_eq!(found_group.name, "Named Group");
    }

    #[tokio::test]
    async fn test_update_group() {
        let storage = setup_test_db().await;

        let now = OffsetDateTime::now_utc();
        let group = Group {
            id: 0,
            name: "Original Name".to_string(),
            description: None,
            created_at: now,
        };

        let repo = GroupRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&group, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let updated_group = Group {
            id,
            name: "Updated Name".to_string(),
            description: Some("Added description".to_string()),
            created_at: now,
        };

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update(id, &updated_group, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_group = found.unwrap();
        assert_eq!(found_group.name, "Updated Name");
        assert_eq!(
            found_group.description,
            Some("Added description".to_string())
        );
    }

    #[tokio::test]
    async fn test_delete_group() {
        let storage = setup_test_db().await;

        let now = OffsetDateTime::now_utc();
        let group = Group {
            id: 0,
            name: "To Delete".to_string(),
            description: None,
            created_at: now,
        };

        let repo = GroupRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&group, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_none());
    }
}
