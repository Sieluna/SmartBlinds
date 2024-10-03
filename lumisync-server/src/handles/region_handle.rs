use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{middleware, Extension, Json, Router};
use lumisync_api::restful::*;

use crate::middlewares::{auth, TokenState};
use crate::models::Region;
use crate::repositories::*;
use crate::services::{Permission, PermissionService, ResourceType, TokenClaims};

#[derive(Clone)]
pub struct RegionState {
    pub user_region_repository: Arc<UserRegionRepository>,
    pub region_repository: Arc<RegionRepository>,
    pub group_repository: Arc<GroupRepository>,
    pub device_repository: Arc<DeviceRepository>,
    pub permission_service: Arc<PermissionService>,
}

pub fn region_router(region_state: RegionState, token_state: TokenState) -> Router {
    Router::new()
        .route(
            "/api/groups/:group_id/regions",
            get(get_regions_by_group_id).post(create_region),
        )
        .route(
            "/api/regions/:region_id",
            get(get_region_by_id)
                .put(update_region)
                .delete(delete_region),
        )
        .route(
            "/api/regions/:region_id/environment",
            put(update_region_environment),
        )
        .route_layer(middleware::from_fn_with_state(token_state, auth))
        .with_state(region_state)
}

#[utoipa::path(
    post,
    path = "/api/groups/{group_id}/regions",
    tag = "region",
    params(
        ("group_id" = i32, Path, description = "Group ID")
    ),
    request_body = CreateRegionRequest,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 201, description = "Region created successfully", body = RegionInfoResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to create region in this group"),
        (status = 404, description = "Group not found"),
        (status = 409, description = "Region name already exists"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_region(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<RegionState>,
    Path(group_id): Path<i32>,
    Json(body): Json<CreateRegionRequest>,
) -> Result<Json<RegionInfoResponse>, StatusCode> {
    let current_user_id = token_data.sub;

    // Check if user has permission to manage the group
    let can_manage = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Group,
            group_id,
            Permission::GROUP_MANAGE_SETTINGS,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !can_manage {
        return Err(StatusCode::FORBIDDEN);
    }

    if body.name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if region name already exists
    if let Ok(Some(_)) = state.region_repository.find_by_name(&body.name).await {
        return Err(StatusCode::CONFLICT);
    }

    // Create region
    let region = Region {
        id: 0,
        group_id,
        name: body.name,
        light: 0,         // Default initial value
        temperature: 0.0, // Default initial value
        humidity: 0.0,    // Default initial value
        is_public: false, // Default to private region
    };

    let mut tx = state
        .region_repository
        .get_pool()
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let region_id = state
        .region_repository
        .create(&region, &mut tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get created region
    let created_region = state
        .region_repository
        .find_by_id(region_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let region_response = RegionInfoResponse {
        id: created_region.id,
        group_id: created_region.group_id,
        name: created_region.name,
    };

    Ok(Json(region_response))
}

#[utoipa::path(
    get,
    path = "/api/groups/{group_id}/regions",
    tag = "region",
    params(
        ("group_id" = i32, Path, description = "Group ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Successfully retrieved region list", body = Vec<RegionInfoResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to access regions in this group"),
        (status = 404, description = "Group not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_regions_by_group_id(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<RegionState>,
    Path(group_id): Path<i32>,
) -> Result<Json<Vec<RegionInfoResponse>>, StatusCode> {
    let current_user_id = token_data.sub;

    // Check if user has permission to view the group
    let can_view = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Group,
            group_id,
            Permission::VIEW,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !can_view {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get all regions in the group
    let regions = state
        .region_repository
        .find_by_group_id(group_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Convert to API response format
    let region_responses: Vec<RegionInfoResponse> = regions
        .into_iter()
        .map(|region| RegionInfoResponse {
            id: region.id,
            group_id: region.group_id,
            name: region.name,
        })
        .collect();

    Ok(Json(region_responses))
}

#[utoipa::path(
    get,
    path = "/api/regions/{region_id}",
    tag = "region",
    params(
        ("region_id" = i32, Path, description = "Region ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Successfully retrieved region details", body = RegionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to access this region"),
        (status = 404, description = "Region not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_region_by_id(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<RegionState>,
    Path(region_id): Path<i32>,
) -> Result<Json<RegionResponse>, StatusCode> {
    let current_user_id = token_data.sub;

    // Use permission service to check if user can access this region
    let can_access = state
        .permission_service
        .can_user_access_region(current_user_id, region_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !can_access {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get region information
    let region = state
        .region_repository
        .find_by_id(region_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Use helper function to build response
    let response = build_region_response(&state, &region).await?;

    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/regions/{region_id}",
    tag = "region",
    params(
        ("region_id" = i32, Path, description = "Region ID")
    ),
    request_body = UpdateRegionSettingRequest,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Region updated successfully", body = RegionResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to modify this region"),
        (status = 404, description = "Region not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_region(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<RegionState>,
    Path(region_id): Path<i32>,
    Json(body): Json<UpdateRegionSettingRequest>,
) -> Result<Json<RegionResponse>, StatusCode> {
    let current_user_id = token_data.sub;

    // Check if user has permission to update region settings
    let can_update = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Region,
            region_id,
            Permission::UPDATE,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !can_update {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get current region information
    let mut region = state
        .region_repository
        .find_by_id(region_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Update region information
    if let Some(name) = body.name {
        region.name = name;
    }

    // Update region
    let mut tx = state
        .region_repository
        .get_pool()
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state
        .region_repository
        .update(region_id, &region, &mut tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get updated region information
    let updated_region = state
        .region_repository
        .find_by_id(region_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Use helper function to build response
    let response = build_region_response(&state, &updated_region).await?;

    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/api/regions/{region_id}",
    tag = "region",
    params(
        ("region_id" = i32, Path, description = "Region ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 204, description = "Region deleted successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to delete this region"),
        (status = 404, description = "Region not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn delete_region(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<RegionState>,
    Path(region_id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    let current_user_id = token_data.sub;

    // Check if user has permission to delete the region
    let can_delete = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Region,
            region_id,
            Permission::DELETE,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !can_delete {
        return Err(StatusCode::FORBIDDEN);
    }

    // Check if region exists
    let _ = state
        .region_repository
        .find_by_id(region_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Delete region
    let mut tx = state
        .region_repository
        .get_pool()
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state
        .region_repository
        .delete(region_id, &mut tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/api/regions/{region_id}/environment",
    tag = "region",
    params(
        ("region_id" = i32, Path, description = "Region ID")
    ),
    request_body = serde_json::Value,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Region environment data updated successfully", body = RegionResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to modify this region"),
        (status = 404, description = "Region not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_region_environment(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<RegionState>,
    Path(region_id): Path<i32>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<RegionResponse>, StatusCode> {
    let current_user_id = token_data.sub;

    // Check if user has permission to update region environment
    let can_update = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Region,
            region_id,
            Permission::UPDATE,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !can_update {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get current region information
    let region = state
        .region_repository
        .find_by_id(region_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Parse environment data
    let light = body
        .get("light")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .unwrap_or(region.light);

    let temperature = body
        .get("temperature")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
        .unwrap_or(region.temperature);

    let humidity = body
        .get("humidity")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
        .unwrap_or(region.humidity);

    // Update region environment data
    let mut tx = state
        .region_repository
        .get_pool()
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state
        .region_repository
        .update_environment_data(region_id, light, temperature, humidity, &mut tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get latest region data
    let updated_region = state
        .region_repository
        .find_by_id(region_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Use helper function to build response
    let response = build_region_response(&state, &updated_region).await?;

    Ok(Json(response))
}

/// Helper function to build region response, reducing code duplication
async fn build_region_response(
    state: &RegionState,
    region: &Region,
) -> Result<RegionResponse, StatusCode> {
    // Get all devices in the region
    let devices = state
        .device_repository
        .find_by_region_id(region.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get user roles in the region
    let user_roles = state
        .user_region_repository
        .get_region_roles_by_region_id(region.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Convert devices to API response format
    let device_responses: Vec<DeviceInfoResponse> = devices
        .into_iter()
        .map(|device| DeviceInfoResponse {
            id: device.id,
            region_id: device.region_id,
            name: device.name,
            device_type: device.device_type,
            location: device.location.clone(),
            status: device.status.clone(),
        })
        .collect();

    // Create region response
    let region_response = RegionResponse {
        info: RegionInfoResponse {
            id: region.id,
            group_id: region.group_id,
            name: region.name.clone(),
        },
        light: region.light,
        temperature: region.temperature,
        humidity: region.humidity,
        users: user_roles,
        devices: device_responses,
    };

    Ok(region_response)
}
