use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub value: CacheValue,
    pub expires_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub last_accessed: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Json(serde_json::Value),
    Bytes(Vec<u8>),
}

impl From<&str> for CacheValue {
    fn from(value: &str) -> Self {
        CacheValue::String(value.to_string())
    }
}

impl From<String> for CacheValue {
    fn from(value: String) -> Self {
        CacheValue::String(value)
    }
}

impl From<i64> for CacheValue {
    fn from(value: i64) -> Self {
        CacheValue::Integer(value)
    }
}

impl From<f64> for CacheValue {
    fn from(value: f64) -> Self {
        CacheValue::Float(value)
    }
}

impl From<bool> for CacheValue {
    fn from(value: bool) -> Self {
        CacheValue::Boolean(value)
    }
}

impl From<serde_json::Value> for CacheValue {
    fn from(value: serde_json::Value) -> Self {
        CacheValue::Json(value)
    }
}

impl From<Vec<u8>> for CacheValue {
    fn from(value: Vec<u8>) -> Self {
        CacheValue::Bytes(value)
    }
}

pub struct CacheService {
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    default_ttl: Option<Duration>,
}

impl CacheService {
    pub fn new(default_ttl: Option<Duration>) -> Self {
        let service = Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
        };

        service.start_cleanup_task();

        service
    }

    pub async fn set<T: Into<CacheValue>>(&self, key: &str, value: T, ttl: Option<Duration>) {
        let now = OffsetDateTime::now_utc();
        let expires_at = ttl
            .or(self.default_ttl)
            .map(|d| now + time::Duration::milliseconds(d.as_millis() as i64));

        let entry = CacheEntry {
            value: value.into(),
            expires_at,
            created_at: now,
            last_accessed: now,
        };

        let mut cache = self.cache.write().await;
        cache.insert(key.to_string(), entry);
    }

    pub async fn get(&self, key: &str) -> Option<CacheValue> {
        let mut cache = self.cache.write().await;

        if let Some(entry) = cache.get_mut(key) {
            if let Some(expires_at) = entry.expires_at {
                if OffsetDateTime::now_utc() > expires_at {
                    cache.remove(key);
                    return None;
                }
            }

            entry.last_accessed = OffsetDateTime::now_utc();

            Some(entry.value.clone())
        } else {
            None
        }
    }

    pub async fn delete(&self, key: &str) -> bool {
        let mut cache = self.cache.write().await;
        cache.remove(key).is_some()
    }

    pub async fn get_string(&self, key: &str) -> Option<String> {
        match self.get(key).await {
            Some(CacheValue::String(value)) => Some(value),
            _ => None,
        }
    }

    pub async fn get_integer(&self, key: &str) -> Option<i64> {
        match self.get(key).await {
            Some(CacheValue::Integer(value)) => Some(value),
            _ => None,
        }
    }

    pub async fn get_float(&self, key: &str) -> Option<f64> {
        match self.get(key).await {
            Some(CacheValue::Float(value)) => Some(value),
            _ => None,
        }
    }

    pub async fn get_boolean(&self, key: &str) -> Option<bool> {
        match self.get(key).await {
            Some(CacheValue::Boolean(value)) => Some(value),
            _ => None,
        }
    }

    pub async fn get_json(&self, key: &str) -> Option<serde_json::Value> {
        match self.get(key).await {
            Some(CacheValue::Json(value)) => Some(value),
            _ => None,
        }
    }

    pub async fn get_bytes(&self, key: &str) -> Option<Vec<u8>> {
        match self.get(key).await {
            Some(CacheValue::Bytes(value)) => Some(value),
            _ => None,
        }
    }

    pub async fn exists(&self, key: &str) -> bool {
        let cache = self.cache.read().await;

        if let Some(entry) = cache.get(key) {
            if let Some(expires_at) = entry.expires_at {
                if OffsetDateTime::now_utc() > expires_at {
                    return false;
                }
            }

            true
        } else {
            false
        }
    }

    pub async fn get_many(&self, keys: &[&str]) -> HashMap<String, CacheValue> {
        let mut result = HashMap::new();

        for key in keys {
            if let Some(value) = self.get(key).await {
                result.insert(key.to_string(), value);
            }
        }

        result
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    pub async fn len(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    fn start_cleanup_task(&self) {
        let cache = self.cache.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                let now = OffsetDateTime::now_utc();
                let mut to_remove = Vec::new();

                {
                    let cache_guard = cache.read().await;
                    for (key, entry) in cache_guard.iter() {
                        if let Some(expires_at) = entry.expires_at {
                            if now > expires_at {
                                to_remove.push(key.clone());
                            }
                        }
                    }
                }

                if !to_remove.is_empty() {
                    let mut cache_guard = cache.write().await;
                    for key in to_remove {
                        cache_guard.remove(&key);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::time::sleep;

    use super::*;

    #[tokio::test]
    async fn test_set_get() {
        let cache = CacheService::new(None);

        cache.set("key1", "value1", None).await;

        let value = cache.get("key1").await;
        assert!(value.is_some());
        if let Some(CacheValue::String(s)) = value {
            assert_eq!(s, "value1");
        } else {
            panic!("Expected String value");
        }

        let value = cache.get("non_existent").await;
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_expiration() {
        let cache = CacheService::new(None);

        cache
            .set("key2", "value2", Some(Duration::from_millis(100)))
            .await;

        let value = cache.get("key2").await;
        assert!(value.is_some());

        sleep(Duration::from_millis(150)).await;

        let value = cache.get("key2").await;
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_delete() {
        let cache = CacheService::new(None);

        cache.set("key3", "value3", None).await;

        let deleted = cache.delete("key3").await;
        assert!(deleted);

        let value = cache.get("key3").await;
        assert!(value.is_none());

        let deleted = cache.delete("non_existent").await;
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = CacheService::new(None);

        cache.set("key4", "value4", None).await;
        cache.set("key5", 123, None).await;

        cache.clear().await;

        let value1 = cache.get("key4").await;
        let value2 = cache.get("key5").await;
        assert!(value1.is_none());
        assert!(value2.is_none());
    }

    #[tokio::test]
    async fn test_typed_getters() {
        let cache = CacheService::new(None);

        cache.set("string", "hello", None).await;
        cache.set("integer", 42i64, None).await;
        cache.set("float", 3.14, None).await;
        cache.set("boolean", true, None).await;
        cache.set("json", json!({"name": "test"}), None).await;
        cache.set("bytes", vec![1, 2, 3], None).await;

        assert_eq!(cache.get_string("string").await, Some("hello".to_string()));
        assert_eq!(cache.get_integer("integer").await, Some(42));
        assert_eq!(cache.get_float("float").await, Some(3.14));
        assert_eq!(cache.get_boolean("boolean").await, Some(true));
        assert_eq!(cache.get_json("json").await, Some(json!({"name": "test"})));
        assert_eq!(cache.get_bytes("bytes").await, Some(vec![1, 2, 3]));
    }
}
