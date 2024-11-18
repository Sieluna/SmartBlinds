use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};

use super::LocalStorage;

pub struct MemoryStorage {
    data: BTreeMap<String, String>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }
}

impl LocalStorage for MemoryStorage {
    type Error = ();

    async fn get_item(&self, key: &str) -> Result<Option<String>, Self::Error> {
        Ok(self.data.get(key).cloned())
    }

    async fn set_item(&mut self, key: &str, value: &str) -> Result<(), Self::Error> {
        self.data.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn remove_item(&mut self, key: &str) -> Result<(), Self::Error> {
        self.data.remove(key);
        Ok(())
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.data.clear();
        Ok(())
    }
}
