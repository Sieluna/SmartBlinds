#[cfg(target_os = "windows")]
mod win;

#[cfg(target_os = "windows")]
pub use win::Backend;

use crate::error::Result;

pub use super::*;

#[async_trait::async_trait]
pub trait WifiBackend: Send + Sync {
    /// Scan surrounding APs
    async fn scan(&self) -> Result<Vec<Network>>;

    /// Connect to an AP (profile will be created/updated)
    async fn connect(&self, creds: &Credentials) -> Result<ConnectionInfo>;

    /// Disconnect from current AP
    async fn disconnect(&self) -> Result<()>;

    /// Currently connected AP
    async fn current_connection(&self) -> Result<Option<ConnectionInfo>>;

    /// Saved profiles on the machine
    async fn get_profiles(&self) -> Result<Vec<Credentials>>;
}
