use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{middleware, Extension, Json, Router};
use lumisync_api::restful::*;
use time::OffsetDateTime;

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
) -> Result<Json<GroupResponse>, StatusCode> {
    let current_user_id = token_data.sub;

    // Use permission service to check if user is an administrator
    let is_admin = state
        .permission_service
        .is_admin(current_user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !is_admin {
        return Err(StatusCode::FORBIDDEN);
    }

    if body.name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if group name already exists
    if let Ok(Some(_)) = state.group_repository.find_by_name(&body.name).await {
        return Err(StatusCode::CONFLICT);
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

    let mut tx = state
        .user_repository
        .get_pool()
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let group_id = state
        .group_repository
        .create_with_user(&group, users_to_add, &mut tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get created group
    let created_group = state
        .group_repository
        .find_by_id(group_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

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
) -> Result<Json<Vec<GroupResponse>>, StatusCode> {
    let current_user_id = token_data.sub;

    // Get groups where the user is a member
    let groups = state
        .group_repository
        .find_by_user_id(current_user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut group_responses = Vec::with_capacity(groups.len());

    for group in groups {
        // Get group's region list
        let regions = state
            .region_repository
            .find_by_group_id(group.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
) -> Result<Json<GroupResponse>, StatusCode> {
    let current_user_id = token_data.sub;

    // Check permissions
    if !state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Group,
            group_id,
            Permission::VIEW,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get group information
    let group = state
        .group_repository
        .find_by_id(group_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get group's region list
    let regions = state
        .region_repository
        .find_by_group_id(group_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
) -> Result<Json<GroupResponse>, StatusCode> {
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
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !can_update {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get existing group
    let mut group = state
        .group_repository
        .find_by_id(group_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Update group information
    group.name = body.name.clone();
    group.description = body.description.clone();

    let mut tx = state
        .user_repository
        .get_pool()
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state
        .group_repository
        .update(group_id, &group, &mut tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get group's region list
    let regions = state
        .region_repository
        .find_by_group_id(group_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
) -> Result<StatusCode, StatusCode> {
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
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !can_delete {
        return Err(StatusCode::FORBIDDEN);
    }

    // Check if group exists
    let _ = state
        .group_repository
        .find_by_id(group_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut tx = state
        .user_repository
        .get_pool()
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state
        .group_repository
        .delete(group_id, &mut tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}
