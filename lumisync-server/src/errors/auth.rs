use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Email already exists")]
    EmailExists,

    #[error("User not found")]
    UserNotFound,

    #[error("Invalid password")]
    InvalidPassword,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Token has expired")]
    TokenExpired,

    #[error("Insufficient permission")]
    InsufficientPermission,

    #[error("Invalid request parameters")]
    InvalidRequest,
}

impl AuthError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            AuthError::EmailExists => StatusCode::CONFLICT,
            AuthError::UserNotFound => StatusCode::NOT_FOUND,
            AuthError::InvalidPassword => StatusCode::UNAUTHORIZED,
            AuthError::InvalidToken => StatusCode::BAD_REQUEST,
            AuthError::TokenExpired => StatusCode::UNAUTHORIZED,
            AuthError::InsufficientPermission => StatusCode::FORBIDDEN,
            AuthError::InvalidRequest => StatusCode::BAD_REQUEST,
        }
    }
}
