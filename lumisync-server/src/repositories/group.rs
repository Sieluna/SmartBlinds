use std::sync::Arc;

use sqlx::{Error, Pool, Sqlite, Transaction};

use crate::configs::Storage;
use crate::models::Group;

#[derive(Clone)]
pub struct GroupRepository {
    storage: Arc<Storage>,
}

impl GroupRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub fn get_pool(&self) -> &Pool<Sqlite> {
        self.storage.get_pool()
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

    pub async fn create_with_user(
        &self,
        item: &Group,
        user_ids: Vec<i32>,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let group_id = sqlx::query(
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

        for user_id in user_ids {
            sqlx::query(
                r#"
                INSERT INTO users_groups_link (user_id, group_id)
                VALUES ($1, $2)
                "#,
            )
            .bind(user_id)
            .bind(group_id)
            .execute(&mut **transaction)
            .await?;
        }

        Ok(group_id as i32)
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

    pub async fn find_all(&self) -> Result<Vec<Group>, Error> {
        let groups: Vec<Group> = sqlx::query_as("SELECT * FROM groups")
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
    use lumisync_api::UserRole;

    use crate::repositories::tests::*;

    use super::*;

    #[tokio::test]
    async fn test_find_group_by_id() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;

        let repo = GroupRepository::new(storage.clone());
        let found = repo.find_by_id(group.id).await.unwrap();
        assert!(found.is_some());

        let found_group = found.unwrap();
        assert_eq!(found_group.name, group.name);
    }

    #[tokio::test]
    async fn test_find_group_by_name() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;

        let repo = GroupRepository::new(storage.clone());
        let found = repo.find_by_name(&group.name).await.unwrap();
        assert!(found.is_some());

        let found_group = found.unwrap();
        assert_eq!(found_group.name, group.name);
    }

    #[tokio::test]
    async fn test_find_by_user_id() {
        let storage = setup_test_db().await;
        let user = create_test_user(storage.clone(), "test@test.com", "test", false).await;
        let group1 = create_test_group(storage.clone(), "test_group_1").await;
        let group2 = create_test_group(storage.clone(), "test_group_2").await;
        create_test_user_group(storage.clone(), user.id, group1.id, true).await;
        create_test_user_group(storage.clone(), user.id, group2.id, false).await;

        let repo = GroupRepository::new(storage.clone());
        let found_groups = repo.find_by_user_id(user.id).await.unwrap();
        assert_eq!(found_groups.len(), 1);

        let group_names: Vec<String> = found_groups.iter().map(|g| g.name.clone()).collect();
        assert!(group_names.contains(&group1.name));
        assert!(!group_names.contains(&group2.name));
    }

    #[tokio::test]
    async fn test_update_group() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;

        let repo = GroupRepository::new(storage.clone());
        let updated = Group {
            id: group.id,
            name: "updated_group".into(),
            description: Some("added_description".into()),
            created_at: group.created_at,
        };

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update(group.id, &updated, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(group.id).await.unwrap();
        assert!(found.is_some());

        let found_group = found.unwrap();
        assert_eq!(found_group.name, updated.name);
        assert_eq!(found_group.description, updated.description);
    }

    #[tokio::test]
    async fn test_delete_group() {
        let storage = setup_test_db().await;
        let group = create_test_group(storage.clone(), "test_group").await;

        let repo = GroupRepository::new(storage.clone());

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(group.id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(group.id).await.unwrap();
        assert!(found.is_none());
    }
}
