use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::Table;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserGroup {
    pub id: i32,
    pub user_id: i32,
    pub group_id: i32,
    pub joined_at: OffsetDateTime,
    pub is_active: bool,
}

#[derive(Clone)]
pub struct UserGroupTable;

impl Table for UserGroupTable {
    fn name(&self) -> &'static str {
        "users_groups_link"
    }

    fn create(&self) -> String {
        String::from(
            r#"
            CREATE TABLE IF NOT EXISTS users_groups_link (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                group_id INTEGER NOT NULL,
                joined_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
                FOREIGN KEY (group_id) REFERENCES groups (id) ON DELETE CASCADE,
                UNIQUE(user_id, group_id)
            );
            "#,
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS users_groups_link;")
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users", "groups"]
    }
}
