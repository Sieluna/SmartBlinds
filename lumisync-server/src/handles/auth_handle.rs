use std::sync::Arc;

use anyhow::anyhow;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{middleware, Extension, Json, Router};
use lumisync_api::models::*;

use crate::errors::{ApiError, AuthError};
use crate::middlewares::{auth, TokenState};
use crate::models::User;
use crate::repositories::{GroupRepository, UserRepository};
use crate::services::{AuthService, TokenClaims, TokenService};

#[derive(Clone)]
pub struct AuthState {
    pub auth_service: Arc<AuthService>,
    pub token_service: Arc<TokenService>,
    pub user_repository: Arc<UserRepository>,
    pub group_repository: Arc<GroupRepository>,
}

pub fn auth_router(auth_state: AuthState, token_state: TokenState) -> Router {
    Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route(
            "/api/auth/refresh",
            post(refresh_token)
                .route_layer(middleware::from_fn_with_state(token_state.clone(), auth)),
        )
        .route(
            "/api/auth/me",
            get(get_current_user)
                .route_layer(middleware::from_fn_with_state(token_state.clone(), auth)),
        )
        .with_state(auth_state)
}

#[utoipa::path(
    post,
    path = "/api/auth/register",
    tag = "auth",
    request_body = RegisterRequest,
    responses(
        (status = 200, description = "Registration successful, return user token", body = String),
        (status = 409, description = "Email already exists"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn register(
    State(state): State<AuthState>,
    Json(body): Json<RegisterRequest>,
) -> Result<String, ApiError> {
    if let Ok(Some(_)) = state.user_repository.find_by_email(&body.email).await {
        return Err(AuthError::EmailExists.into());
    }

    let hash_password = state
        .auth_service
        .hash(&body.password)
        .map_err(|e| anyhow!("Failed to hash password: {}", e))?;

    let user = User {
        id: 0,
        email: body.email.clone(),
        password: hash_password,
        role: UserRole::User.to_string(),
    };

    let mut tx = state.user_repository.get_pool().begin().await?;

    let id = state.user_repository.create(&user, &mut tx).await?;

    tx.commit().await?;

    let created_user = state
        .user_repository
        .find_by_id(id)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    let token = state
        .token_service
        .generate_token(created_user)
        .map_err(|e| anyhow!("Failed to generate token: {}", e))?
        .token;

    Ok(token)
}

#[utoipa::path(
    post,
    path = "/api/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful, return user token", body = String),
        (status = 404, description = "User not found"),
        (status = 401, description = "Invalid password"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn login(
    State(state): State<AuthState>,
    Json(body): Json<LoginRequest>,
) -> Result<String, ApiError> {
    let user = state
        .user_repository
        .find_by_email(&body.email)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    let result = state
        .auth_service
        .verify(&user, &body.password)
        .map_err(|e| anyhow!("Failed to verify password: {}", e))?;

    if !result {
        return Err(AuthError::InvalidPassword.into());
    }

    let token = state
        .token_service
        .generate_token(user)
        .map_err(|e| anyhow!("Failed to generate token: {}", e))?
        .token;

    Ok(token)
}

#[utoipa::path(
    post,
    path = "/api/auth/refresh",
    tag = "auth",
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Refresh token successful", body = String),
        (status = 400, description = "Invalid token"),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn refresh_token(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<AuthState>,
) -> Result<String, ApiError> {
    let token = state
        .token_service
        .generate_token(token_data)
        .map_err(|e| anyhow!("Failed to generate token: {}", e))?
        .token;

    Ok(token)
}

#[utoipa::path(
    get,
    path = "/api/auth/me",
    tag = "auth",
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Get current user information successfully", body = UserResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "User does not exist"),
        (status = 500, description = "Server internal error")
    )
)]
pub async fn get_current_user(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<AuthState>,
) -> Result<Json<UserResponse>, ApiError> {
    let user = state
        .user_repository
        .find_by_id(token_data.sub)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    let groups = state.group_repository.find_by_user_id(user.id).await?;

    let group_responses: Vec<GroupResponse> = groups
        .into_iter()
        .map(|group| GroupResponse {
            id: group.id,
            name: group.name,
            description: group.description,
            created_at: group.created_at,
            regions: vec![],
        })
        .collect();

    let user_response = UserResponse {
        id: user.id,
        group_id: group_responses.first().map(|g| g.id).unwrap_or(0),
        email: user.email,
        role: user.role.into(),
        groups: group_responses,
    };

    Ok(Json(user_response))
}
