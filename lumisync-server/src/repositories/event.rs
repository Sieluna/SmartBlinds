use std::sync::Arc;

use sqlx::{Error, Pool, Sqlite, Transaction};

use crate::configs::Storage;
use crate::models::Event;

#[derive(Clone)]
pub struct EventRepository {
    storage: Arc<Storage>,
}

impl EventRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub fn get_pool(&self) -> &Pool<Sqlite> {
        self.storage.get_pool()
    }
}

impl EventRepository {
    pub async fn create(
        &self,
        item: &Event,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<i32, Error> {
        let id = sqlx::query(
            r#"
            INSERT INTO events (event_type, payload, time)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(&item.event_type)
        .bind(&item.payload)
        .bind(item.time)
        .execute(&mut **transaction)
        .await?
        .last_insert_rowid();

        Ok(id as i32)
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<Event>, Error> {
        let event: Option<Event> = sqlx::query_as("SELECT * FROM events WHERE id = $1")
            .bind(id)
            .fetch_optional(self.storage.get_pool())
            .await?;

        Ok(event)
    }

    pub async fn find_all(&self) -> Result<Vec<Event>, Error> {
        let events: Vec<Event> = sqlx::query_as("SELECT * FROM events")
            .fetch_all(self.storage.get_pool())
            .await?;

        Ok(events)
    }

    pub async fn update(
        &self,
        id: i32,
        item: &Event,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE events
            SET event_type = $1, payload = $2, time = $3
            WHERE id = $4
            "#,
        )
        .bind(&item.event_type)
        .bind(&item.payload)
        .bind(item.time)
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
        sqlx::query("DELETE FROM events WHERE id = $1")
            .bind(id)
            .execute(&mut **transaction)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use time::OffsetDateTime;

    use crate::tests::*;

    use super::*;

    #[tokio::test]
    async fn test_find_event_by_id() {
        let storage = setup_test_db().await;

        let now = OffsetDateTime::now_utc();
        let event = Event {
            id: 0,
            event_type: "user_login".to_string(),
            payload: json!({"user_id": 1, "ip": "127.0.0.1"}),
            time: now,
        };

        let repo = EventRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&event, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_event = found.unwrap();
        assert_eq!(found_event.event_type, "user_login");

        if let Some(payload) = found_event.payload.as_object() {
            assert_eq!(payload.get("user_id").unwrap().as_i64().unwrap(), 1);
            assert_eq!(payload.get("ip").unwrap().as_str().unwrap(), "127.0.0.1");
        } else {
            panic!("Payload is not an object");
        }
    }

    #[tokio::test]
    async fn test_update_event() {
        let storage = setup_test_db().await;

        let now = OffsetDateTime::now_utc();
        let event = Event {
            id: 0,
            event_type: "sensor_reading".to_string(),
            payload: json!({"sensor_id": 2, "temperature": 22.5}),
            time: now,
        };

        let repo = EventRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&event, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let updated_time = OffsetDateTime::now_utc();
        let updated_event = Event {
            id,
            event_type: "updated_sensor_reading".to_string(),
            payload: json!({"sensor_id": 2, "temperature": 23.5, "updated": true}),
            time: updated_time,
        };

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.update(id, &updated_event, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found_event = found.unwrap();
        assert_eq!(found_event.event_type, "updated_sensor_reading");

        if let Some(payload) = found_event.payload.as_object() {
            assert_eq!(payload.get("temperature").unwrap().as_f64().unwrap(), 23.5);
            assert!(payload.get("updated").unwrap().as_bool().unwrap());
        } else {
            panic!("Payload is not an object");
        }
    }

    #[tokio::test]
    async fn test_delete_event() {
        let storage = setup_test_db().await;

        let now = OffsetDateTime::now_utc();
        let event = Event {
            id: 0,
            event_type: "window_open".to_string(),
            payload: json!({"window_id": 3}),
            time: now,
        };

        let repo = EventRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        let id = repo.create(&event, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let mut tx = storage.get_pool().begin().await.unwrap();
        repo.delete(id, &mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_find_all_events() {
        let storage = setup_test_db().await;

        let now = OffsetDateTime::now_utc();
        let events = vec![
            Event {
                id: 0,
                event_type: "event_one".to_string(),
                payload: json!({"data": "first event"}),
                time: now,
            },
            Event {
                id: 0,
                event_type: "event_two".to_string(),
                payload: json!({"data": "second event"}),
                time: now,
            },
        ];

        let repo = EventRepository::new(storage.clone());
        let mut tx = storage.get_pool().begin().await.unwrap();
        for event in &events {
            repo.create(event, &mut tx).await.unwrap();
        }
        tx.commit().await.unwrap();

        let all_events = repo.find_all().await.unwrap();
        assert!(all_events.len() >= 2);

        let event_types: Vec<String> = all_events.iter().map(|e| e.event_type.clone()).collect();
        assert!(event_types.contains(&"event_one".to_string()));
        assert!(event_types.contains(&"event_two".to_string()));
    }
}
