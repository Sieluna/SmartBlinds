#[derive(Debug)]
pub enum WifiError {
    Backend(String),
    NotFound(String),
    Connect(String),
}

pub type Result<T, E = WifiError> = std::result::Result<T, E>;

impl std::fmt::Display for WifiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WifiError::Backend(e) => write!(f, "Backend error: {}", e),
            WifiError::NotFound(e) => write!(f, "Target network \"{}\" not found", e),
            WifiError::Connect(e) => write!(f, "Connection failed: {}", e),
        }
    }
}

impl std::error::Error for WifiError {}
