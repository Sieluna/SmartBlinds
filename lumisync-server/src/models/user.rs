use std::fmt;

use serde::{Deserialize, Serialize};

use super::Table;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    #[default]
    User,
}

impl From<String> for Role {
    fn from(value: String) -> Self {
        match value.as_str() {
            "admin" => Role::Admin,
            _ => Role::User,
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::User => write!(f, "user"),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: i32,
    pub group_id: i32,
    pub email: String,
    pub password: String,
    pub role: String,
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
                group_id INTEGER NOT NULL,
                email VARCHAR(255) NOT NULL UNIQUE,
                password VARCHAR(255) NOT NULL,
                role VARCHAR(20) NOT NULL,
                FOREIGN KEY (group_id) REFERENCES groups (id) ON DELETE CASCADE
            );
            "#,
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS users;")
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["groups"]
    }
}
