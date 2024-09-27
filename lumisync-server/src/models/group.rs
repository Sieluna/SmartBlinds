use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::Table;

#[derive(Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Group {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub created_at: OffsetDateTime,
}

#[derive(Clone)]
pub struct GroupTable;

impl Table for GroupTable {
    fn name(&self) -> &'static str {
        "groups"
    }

    fn create(&self) -> String {
        String::from(
            r#"
            CREATE TABLE IF NOT EXISTS groups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name VARCHAR(255) NOT NULL UNIQUE,
                description TEXT,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS groups;")
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }
}
