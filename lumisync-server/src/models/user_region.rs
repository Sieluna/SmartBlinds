use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::Table;

#[derive(Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserRegion {
    pub id: i32,
    pub user_id: i32,
    pub region_id: i32,
    pub role: String,
    pub joined_at: OffsetDateTime,
    pub is_active: bool,
}

#[derive(Clone)]
pub struct UserRegionTable;

impl Table for UserRegionTable {
    fn name(&self) -> &'static str {
        "users_regions_link"
    }

    fn create(&self) -> String {
        String::from(
            r#"
            CREATE TABLE IF NOT EXISTS users_regions_link (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                region_id INTEGER NOT NULL,
                role VARCHAR(255) NOT NULL DEFAULT 'visitor',
                joined_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
                FOREIGN KEY (region_id) REFERENCES regions (id) ON DELETE CASCADE
            );
            "#,
        )
    }

    fn dispose(&self) -> String {
        String::from("DROP TABLE IF EXISTS users_regions_link;")
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users", "regions"]
    }
}
