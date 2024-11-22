pub mod api;
pub mod auth;
pub mod device;
pub mod group;
pub mod message;
pub mod region;
pub mod setting;

pub use api::ApiError;
pub use auth::AuthError;
pub use device::DeviceError;
pub use group::GroupError;
pub use message::MessageError;
pub use region::RegionError;
pub use setting::SettingError;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use uuid::Uuid;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Extract status code and error message from the specific error type
        let (status, error_message, log_message) = match self {
            ApiError::AuthError(e) => (e.status_code(), e.to_string(), None),
            ApiError::DeviceError(e) => (e.status_code(), e.to_string(), None),
            ApiError::GroupError(e) => (e.status_code(), e.to_string(), None),
            ApiError::MessageError(e) => (e.status_code(), e.to_string(), None),
            ApiError::RegionError(e) => (e.status_code(), e.to_string(), None),
            ApiError::SettingError(e) => (e.status_code(), e.to_string(), None),
            ApiError::DatabaseError(e) => {
                let error_id = Uuid::new_v4();
                tracing::error!(error_id = ?error_id, "Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                    Some(error_id.to_string()),
                )
            }
            ApiError::InternalError(e) => {
                let error_id = Uuid::new_v4();
                tracing::error!(error_id = ?error_id, "Internal error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                    Some(error_id.to_string()),
                )
            }
        };

        // Create a consistent JSON error response
        let mut error_obj = json!({
            "code": status.as_u16(),
            "message": error_message
        });

        // Add error_id if available (for internal errors)
        if let Some(error_id) = log_message {
            error_obj["error_id"] = json!(error_id);
        }

        let body = Json(json!({
            "error": error_obj
        }));

        // Combine status code and JSON body into a response
        (status, body).into_response()
    }
}
