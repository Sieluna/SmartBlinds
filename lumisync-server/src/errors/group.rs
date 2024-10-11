use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum GroupError {
    #[error("Group not found")]
    GroupNotFound,

    #[error("Group name already exists")]
    GroupNameExists,

    #[error("Invalid request parameters")]
    InvalidRequest,

    #[error("Insufficient permission")]
    InsufficientPermission,

    #[error("User not in group")]
    UserNotInGroup,

    #[error("Group user limit exceeded")]
    GroupUserLimitExceeded,
}

impl GroupError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            GroupError::GroupNotFound => StatusCode::NOT_FOUND,
            GroupError::GroupNameExists => StatusCode::CONFLICT,
            GroupError::InvalidRequest => StatusCode::BAD_REQUEST,
            GroupError::InsufficientPermission => StatusCode::FORBIDDEN,
            GroupError::UserNotInGroup => StatusCode::FORBIDDEN,
            GroupError::GroupUserLimitExceeded => StatusCode::BAD_REQUEST,
        }
    }
}
