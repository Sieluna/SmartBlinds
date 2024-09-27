use std::sync::Arc;

use sqlx::{Error, Pool, Sqlite, Transaction};

use crate::configs::Storage;
use crate::models::User;

#[derive(Clone)]
pub struct UserRepository {
    storage: Arc<Storage>,
}

impl UserRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub fn get_pool(&self) -> &Pool<Sqlite> {
        self.storage.get_pool()
    }
}

impl UserRepository {
    pub async fn create(
        &self,
        item: &User,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO users (email, password, role)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(&item.email)
        .bind(&item.password)
        .bind(&item.role)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<User>, Error> {
        let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(user)
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, Error> {
        let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE email = $1")
            .bind(email)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(user)
    }

    pub async fn find_all(&self) -> Result<Vec<User>, Error> {
        let users: Vec<User> = sqlx::query_as("SELECT * FROM users")
            .fetch_all(self.storage.get_pool())
            .await?;

        Ok(users)
    }

    pub async fn find_all_by_group_id(&self, group_id: i32) -> Result<Vec<User>, Error> {
        let users: Vec<User> = sqlx::query_as(
            r#"
            SELECT u.* FROM users u
            INNER JOIN users_groups_link ugl ON u.id = ugl.user_id
            WHERE ugl.group_id = $1 AND ugl.is_active = TRUE
            "#,
        )
        .bind(group_id)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(users)
    }

    pub async fn find_all_by_region_id(&self, region_id: i32) -> Result<Vec<User>, Error> {
        let users: Vec<User> = sqlx::query_as(
            r#"
            SELECT u.* FROM users u
            INNER JOIN users_regions_link url ON u.id = url.user_id
            WHERE url.region_id = $1 AND url.is_active = TRUE
            "#,
        )
        .bind(region_id)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(users)
    }

    pub async fn update(
        &self,
        id: i32,
        item: &User,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE users
            SET email = $1, password = $2, role = $3
            WHERE id = $4
            "#,
        )
        .bind(&item.email)
        .bind(&item.password)
        .bind(&item.role)
        .bind(id)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    pub async fn update_role(
        &self,
        id: i32,
        role: &str,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE users
            SET role = $1
            WHERE id = $2
            "#,
        )
        .bind(role)
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
        sqlx::query("DELETE FROM users WHERE id = $1")
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
    async fn test_find_user_by_id() {
        let storage = setup_test_db().await;
        let user = create_test_user(storage.clone(), "test@test.com", "test", false).await;

        let repo = UserRepository::new(storage.clone());
        let found = repo.find_by_id(user.id).await.unwrap();
        assert!(found.is_some());

        let found_user = found.unwrap();
        assert_eq!(found_user.email, user.email);
        assert_eq!(found_user.password, user.password);
    }

    #[tokio::test]
    async fn test_find_user_by_email() {
        let storage = setup_test_db().await;
        let user = create_test_user(storage.clone(), "test@test.com", "test", false).await;

        let repo = UserRepository::new(storage.clone());
        let found = repo.find_by_email(&user.email).await.unwrap();
        assert!(found.is_some());

        let found_user = found.unwrap();
        assert_eq!(found_user.email, user.email);
    }

    #[tokio::test]
    async fn test_update_user() {
        let storage = setup_test_db().await;
        let user = create_test_user(storage.clone(), "test@test.com", "test", false).await;

        let repo = UserRepository::new(storage.clone());
        let updated_user = User {
            id: user.id,
            email: "updated@test.com".to_string(),
            password: "updated_test".to_string(),
            role: "user".to_string(),
        };

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update(user.id, &updated_user, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(user.id).await.unwrap();
        assert!(found.is_some());

        let found_user = found.unwrap();
        assert_eq!(found_user.email, updated_user.email);
        assert_eq!(found_user.password, updated_user.password);
    }

    #[tokio::test]
    async fn test_update_role() {
        let storage = setup_test_db().await;
        let user = create_test_user(storage.clone(), "test@test.com", "test", false).await;

        let repo = UserRepository::new(storage.clone());

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update_role(user.id, "admin", &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(user.id).await.unwrap();
        assert!(found.is_some());

        let found_user = found.unwrap();
        assert_eq!(found_user.role, "admin");
    }

    #[tokio::test]
    async fn test_delete_user() {
        let storage = setup_test_db().await;
        let user = create_test_user(storage.clone(), "test@test.com", "test", false).await;

        let repo = UserRepository::new(storage.clone());

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(user.id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(user.id).await.unwrap();
        assert!(found.is_none());
    }
}
