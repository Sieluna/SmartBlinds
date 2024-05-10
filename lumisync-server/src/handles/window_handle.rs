use std::sync::Arc;

use axum::{Extension, Json};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::configs::storage::Storage;
use crate::models::user::{Role, User};
use crate::models::window::Window;
use crate::services::actuator_service::ActuatorService;
use crate::services::token_service::TokenClaims;

#[derive(Serialize, Deserialize, Clone)]
pub struct WindowBody {
    pub name: String,
    pub state: f32,
}

#[derive(Clone)]
pub struct WindowState {
    pub actuator_service: Option<Arc<ActuatorService>>,
    pub storage: Arc<Storage>,
}

pub async fn create_window(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<WindowState>,
    Json(body): Json<WindowBody>,
) -> Result<impl IntoResponse, StatusCode> {
    match Role::from(token_data.role.clone()) {
        Role::Admin => {
            let window = sqlx::query_as::<_, Window>(
                r#"
                INSERT INTO windows (group_id, name, state)
                    VALUES ($1, $2, $3)
                    RETURNING *;
                "#
            )
                .bind(&token_data.group_id)
                .bind(&body.name)
                .bind(body.state)
                .fetch_one(state.storage.get_pool())
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            Ok(Json(window))
        },
        Role::User => Err(StatusCode::FORBIDDEN),
    }
}

pub async fn get_windows(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<WindowState>
) -> Result<impl IntoResponse, StatusCode> {
    let windows = sqlx::query_as::<_, Window>(
        r#"
            SELECT w.* FROM users u
                JOIN users_windows_link uw ON u.id = uw.user_id
                JOIN windows w ON uw.window_id = w.id
                WHERE u.id = $1;
        "#
    )
        .bind(&token_data.sub)
        .fetch_all(state.storage.get_pool())
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(windows))
}

pub async fn get_window_owners(
    Path(window_id): Path<i32>,
    State(state): State<WindowState>
) -> Result<impl IntoResponse, StatusCode> {
    let users = sqlx::query_as::<_, User>(
        r#"
        SELECT u.* FROM users u
            JOIN users_windows_link uw ON u.id = uw.user_id
            JOIN windows w ON uw.window_id = w.id
            WHERE w.id = $1;
        "#
    )
        .bind(window_id.to_string())
        .fetch_all(state.storage.get_pool())
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(users))
}

pub async fn update_window(
    Extension(token_data): Extension<TokenClaims>,
    Path(window_id): Path<i32>,
    State(state): State<WindowState>,
    Json(body): Json<WindowBody>,
) -> Result<impl IntoResponse, StatusCode> {
    let updated_window = sqlx::query_as::<_, Window>(
        r#"
        UPDATE windows SET name = $1, state = $2
            WHERE id = $3
            RETURNING *;
        "#
    )
        .bind(&body.name)
        .bind(&body.state)
        .bind(window_id.to_string())
        .fetch_one(state.storage.get_pool())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(updated_window))
}

pub async fn delete_window(
    Extension(token_data): Extension<TokenClaims>,
    Path(window_id): Path<i32>,
    State(state): State<WindowState>,
) -> Result<impl IntoResponse, StatusCode> {
    match Role::from(token_data.role.clone()) {
        Role::Admin => {
            sqlx::query("DELETE FROM windows WHERE id = $1")
                .bind(window_id.to_string())
                .execute(state.storage.get_pool())
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            Ok(StatusCode::OK)
        },
        Role::User => {
            sqlx::query("DELETE FROM users_windows_link WHERE window_id = $1 AND user_id = $2")
                .bind(window_id.to_string())
                .bind(&token_data.sub)
                .execute(state.storage.get_pool())
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            Ok(StatusCode::OK)
        },
    }
}
