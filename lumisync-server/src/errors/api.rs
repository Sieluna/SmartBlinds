use super::{AuthError, DeviceError, GroupError, RegionError};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Authentication error: {0}")]
    AuthError(#[from] AuthError),

    #[error("Device error: {0}")]
    DeviceError(#[from] DeviceError),

    #[error("Group error: {0}")]
    GroupError(#[from] GroupError),

    #[error("Region error: {0}")]
    RegionError(#[from] RegionError),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Internal error: {0}")]
    InternalError(#[from] anyhow::Error),
}
