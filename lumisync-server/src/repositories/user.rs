use std::sync::Arc;

use sqlx::{Error, Sqlite, Transaction};

use crate::configs::Storage;
use crate::models::User;

pub struct UserRepository {
    storage: Arc<Storage>,
}

impl UserRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }
}

impl UserRepository {
    // Create new user
    pub async fn create(
        &self,
        item: &User,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO users (email, password)
            VALUES ($1, $2)
            "#,
        )
        .bind(&item.email)
        .bind(&item.password)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    // Find user by ID
    pub async fn find_by_id(&self, id: i32) -> Result<Option<User>, Error> {
        let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(user)
    }

    // Find user by email
    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, Error> {
        let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE email = $1")
            .bind(email)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(user)
    }

    // Get all users
    pub async fn find_all(&self) -> Result<Vec<User>, Error> {
        let users: Vec<User> = sqlx::query_as("SELECT * FROM users")
            .fetch_all(self.storage.get_pool())
            .await?;

        Ok(users)
    }

    // Update user information
    pub async fn update(
        &self,
        id: i32,
        item: &User,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE users
            SET email = $1, password = $2
            WHERE id = $3
            "#,
        )
        .bind(&item.email)
        .bind(&item.password)
        .bind(id)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    // Delete user
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
    async fn test_find_user_by_id() {
        let storage = setup_test_db().await;

        let user = User {
            id: 0,
            email: "test@example.com".to_string(),
            password: "hashed_password".to_string(),
        };

        let repo = UserRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&user, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_user = found.unwrap();
        assert_eq!(found_user.email, "test@example.com");
        assert_eq!(found_user.password, "hashed_password");
    }

    #[tokio::test]
    async fn test_find_user_by_email() {
        let storage = setup_test_db().await;

        let user = User {
            id: 0,
            email: "findme@example.com".to_string(),
            password: "secret123".to_string(),
        };

        let repo = UserRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.create(&user, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_email("findme@example.com").await.unwrap();
        assert!(found.is_some());
        let found_user = found.unwrap();
        assert_eq!(found_user.email, "findme@example.com");
    }

    #[tokio::test]
    async fn test_update_user() {
        let storage = setup_test_db().await;

        let user = User {
            id: 0,
            email: "original@example.com".to_string(),
            password: "original_password".to_string(),
        };

        let repo = UserRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&user, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let updated_user = User {
            id,
            email: "updated@example.com".to_string(),
            password: "updated_password".to_string(),
        };

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update(id, &updated_user, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_user = found.unwrap();
        assert_eq!(found_user.email, "updated@example.com");
        assert_eq!(found_user.password, "updated_password");
    }

    #[tokio::test]
    async fn test_delete_user() {
        let storage = setup_test_db().await;

        let user = User {
            id: 0,
            email: "delete_me@example.com".to_string(),
            password: "delete_password".to_string(),
        };

        let repo = UserRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&user, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_none());
    }
}
