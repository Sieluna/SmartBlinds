use serde::{Deserialize, Serialize};

use super::Table;

#[derive(Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub password: String,
}

#[derive(Clone)]
pub struct UserTable;

impl Table for UserTable {
    fn name(&self) -> &'static str {
        "users"
    }

    fn create(&self) -> String {
        String::from(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email VARCHAR(255) NOT NULL UNIQUE,
                password VARCHAR(255) NOT NULL
            );
            "#,
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS users;")
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }
}
