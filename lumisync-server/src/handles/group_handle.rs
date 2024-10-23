use std::sync::Arc;

use anyhow::anyhow;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{middleware, Extension, Json, Router};
use lumisync_api::models::*;
use time::OffsetDateTime;

use crate::errors::{ApiError, AuthError, GroupError};
use crate::middlewares::{auth, TokenState};
use crate::models::Group;
use crate::repositories::{GroupRepository, RegionRepository, UserRepository};
use crate::services::{Permission, PermissionService, ResourceType, TokenClaims};

#[derive(Clone)]
pub struct GroupState {
    pub user_repository: Arc<UserRepository>,
    pub group_repository: Arc<GroupRepository>,
    pub region_repository: Arc<RegionRepository>,
    pub permission_service: Arc<PermissionService>,
}

pub fn group_router(group_state: GroupState, token_state: TokenState) -> Router {
    Router::new()
        .route("/api/groups", get(get_user_groups).post(create_group))
        .route(
            "/api/groups/:group_id",
            get(get_group_by_id).put(update_group).delete(delete_group),
        )
        .route(
            "/api/groups/:group_id/users",
            get(get_group_users),
        )
        .route_layer(middleware::from_fn_with_state(token_state, auth))
        .with_state(group_state)
}

#[utoipa::path(
    post,
    path = "/api/groups",
    tag = "group",
    request_body = CreateGroupRequest,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 201, description = "Group created successfully", body = GroupResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Group name already exists"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_group(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<GroupState>,
    Json(body): Json<CreateGroupRequest>,
) -> Result<Json<GroupResponse>, ApiError> {
    let current_user_id = token_data.sub;

    // Use permission service to check if user is an administrator
    let is_admin = state
        .permission_service
        .is_admin(current_user_id)
        .await
        .map_err(|e| anyhow!("Failed to check admin status: {}", e))?;

    if !is_admin {
        return Err(AuthError::InsufficientPermission.into());
    }

    if body.name.is_empty() {
        return Err(GroupError::InvalidRequest.into());
    }

    // Check if group name already exists
    if let Ok(Some(_)) = state.group_repository.find_by_name(&body.name).await {
        return Err(GroupError::GroupNameExists.into());
    }

    // Create group
    let now = OffsetDateTime::now_utc();
    let group = Group {
        id: 0,
        name: body.name.clone(),
        description: body.description.clone(),
        created_at: now,
    };

    let mut users_to_add = body.users.clone();
    users_to_add.push(current_user_id);
    users_to_add.sort_unstable();
    users_to_add.dedup();

    // Check if user count exceeds limit
    if users_to_add.len() > 100 {
        return Err(GroupError::GroupUserLimitExceeded.into());
    }

    let mut tx = state.user_repository.get_pool().begin().await?;

    let group_id = state
        .group_repository
        .create_with_user(&group, users_to_add, &mut tx)
        .await?;

    tx.commit().await?;

    // Get created group
    let created_group = state
        .group_repository
        .find_by_id(group_id)
        .await?
        .ok_or(GroupError::GroupNotFound)?;

    let group_response = GroupResponse {
        id: created_group.id,
        name: created_group.name,
        description: created_group.description,
        created_at: created_group.created_at,
        regions: vec![],
    };

    Ok(Json(group_response))
}

#[utoipa::path(
    get,
    path = "/api/groups",
    tag = "group",
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Successfully retrieved group list", body = Vec<GroupResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_user_groups(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<GroupState>,
) -> Result<Json<Vec<GroupResponse>>, ApiError> {
    let current_user_id = token_data.sub;

    // Get groups where the user is a member
    let groups = state
        .group_repository
        .find_by_user_id(current_user_id)
        .await?;

    let mut group_responses = Vec::with_capacity(groups.len());

    for group in groups {
        // Get group's region list
        let regions = state.region_repository.find_by_group_id(group.id).await?;

        let region_responses: Vec<RegionInfoResponse> = regions
            .into_iter()
            .map(|region| RegionInfoResponse {
                id: region.id,
                group_id: region.group_id,
                name: region.name,
            })
            .collect();

        let group_response = GroupResponse {
            id: group.id,
            name: group.name,
            description: group.description,
            created_at: group.created_at,
            regions: region_responses,
        };

        group_responses.push(group_response);
    }

    Ok(Json(group_responses))
}

#[utoipa::path(
    get,
    path = "/api/groups/{group_id}",
    tag = "group",
    params(
        ("group_id" = i32, Path, description = "Group ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Successfully retrieved group details", body = GroupResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to access this group"),
        (status = 404, description = "Group not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_group_by_id(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<GroupState>,
    Path(group_id): Path<i32>,
) -> Result<Json<GroupResponse>, ApiError> {
    let current_user_id = token_data.sub;

    // Check permissions
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Group,
            group_id,
            Permission::VIEW,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(GroupError::InsufficientPermission.into());
    }

    // Get group information
    let group = state
        .group_repository
        .find_by_id(group_id)
        .await?
        .ok_or(GroupError::GroupNotFound)?;

    // Get group's region list
    let regions = state.region_repository.find_by_group_id(group_id).await?;

    let region_responses: Vec<RegionInfoResponse> = regions
        .into_iter()
        .map(|region| RegionInfoResponse {
            id: region.id,
            group_id: region.group_id,
            name: region.name,
        })
        .collect();

    let group_response = GroupResponse {
        id: group.id,
        name: group.name,
        description: group.description,
        created_at: group.created_at,
        regions: region_responses,
    };

    Ok(Json(group_response))
}

#[utoipa::path(
    put,
    path = "/api/groups/{group_id}",
    tag = "group",
    params(
        ("group_id" = i32, Path, description = "Group ID")
    ),
    request_body = CreateGroupRequest,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Group updated successfully", body = GroupResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to modify this group"),
        (status = 404, description = "Group not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_group(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<GroupState>,
    Path(group_id): Path<i32>,
    Json(body): Json<CreateGroupRequest>,
) -> Result<Json<GroupResponse>, ApiError> {
    let current_user_id = token_data.sub;

    // Use permission service to check if user has permission to manage the group
    let can_update = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Group,
            group_id,
            Permission::GROUP_MANAGE_SETTINGS,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !can_update {
        return Err(GroupError::InsufficientPermission.into());
    }

    if body.name.is_empty() {
        return Err(GroupError::InvalidRequest.into());
    }

    // Get existing group
    let mut group = state
        .group_repository
        .find_by_id(group_id)
        .await?
        .ok_or(GroupError::GroupNotFound)?;

    // Update group information
    group.name = body.name.clone();
    group.description = body.description.clone();

    let mut tx = state.user_repository.get_pool().begin().await?;

    state
        .group_repository
        .update(group_id, &group, &mut tx)
        .await?;

    tx.commit().await?;

    // Get group's region list
    let regions = state.region_repository.find_by_group_id(group_id).await?;

    let region_responses: Vec<RegionInfoResponse> = regions
        .into_iter()
        .map(|region| RegionInfoResponse {
            id: region.id,
            group_id: region.group_id,
            name: region.name,
        })
        .collect();

    let group_response = GroupResponse {
        id: group.id,
        name: group.name,
        description: group.description,
        created_at: group.created_at,
        regions: region_responses,
    };

    Ok(Json(group_response))
}

#[utoipa::path(
    delete,
    path = "/api/groups/{group_id}",
    tag = "group",
    params(
        ("group_id" = i32, Path, description = "Group ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 204, description = "Group deleted successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to delete this group"),
        (status = 404, description = "Group not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn delete_group(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<GroupState>,
    Path(group_id): Path<i32>,
) -> Result<axum::http::StatusCode, ApiError> {
    let current_user_id = token_data.sub;

    // Use permission service to check if user has permission to delete the group
    let can_delete = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Group,
            group_id,
            Permission::DELETE,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !can_delete {
        return Err(GroupError::InsufficientPermission.into());
    }

    // Check if group exists
    let _ = state
        .group_repository
        .find_by_id(group_id)
        .await?
        .ok_or(GroupError::GroupNotFound)?;

    let mut tx = state.user_repository.get_pool().begin().await?;

    state.group_repository.delete(group_id, &mut tx).await?;

    tx.commit().await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/groups/{group_id}/users",
    tag = "group",
    params(
        ("group_id" = i32, Path, description = "Group ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Successfully retrieved group members", body = Vec<UserInfoResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to access this group"),
        (status = 404, description = "Group not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_group_users(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<GroupState>,
    Path(group_id): Path<i32>,
) -> Result<Json<Vec<UserInfoResponse>>, ApiError> {
    let current_user_id = token_data.sub;

    // Check if group exists
    state
        .group_repository
        .find_by_id(group_id)
        .await?
        .ok_or(GroupError::GroupNotFound)?;

    // Check if user has permission to view the group
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Group,
            group_id,
            Permission::VIEW,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(GroupError::InsufficientPermission.into());
    }

    // Get all users in the group
    let users = state
        .user_repository
        .find_all_by_group_id(group_id)
        .await?;

    // Convert to API response format
    let user_responses: Vec<UserInfoResponse> = users
        .into_iter()
        .map(|user| UserInfoResponse {
            id: user.id,
            email: user.email,
            role: user.role.into(),
        })
        .collect();

    Ok(Json(user_responses))
}
