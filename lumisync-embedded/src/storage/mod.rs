mod memory;

pub use memory::*;

use alloc::string::String;

#[allow(async_fn_in_trait)]
pub trait LocalStorage {
    type Error;

    async fn get_item(&self, key: &str) -> Result<Option<String>, Self::Error>;

    async fn set_item(&mut self, key: &str, value: &str) -> Result<(), Self::Error>;

    async fn remove_item(&mut self, key: &str) -> Result<(), Self::Error>;

    async fn clear(&mut self) -> Result<(), Self::Error>;
}
