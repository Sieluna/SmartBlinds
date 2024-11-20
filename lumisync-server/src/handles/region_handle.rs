use std::sync::Arc;

use anyhow::anyhow;
use axum::extract::{Path, State};
use axum::routing::{get, put};
use axum::{Extension, Json, Router, middleware};
use lumisync_api::models::*;

use crate::errors::{ApiError, GroupError, RegionError, SettingError};
use crate::middlewares::{TokenState, auth};
use crate::models::{Region, RegionSetting};
use crate::repositories::*;
use crate::services::{Permission, PermissionService, ResourceType, TokenClaims};

#[derive(Clone)]
pub struct RegionState {
    pub user_region_repository: Arc<UserRegionRepository>,
    pub region_repository: Arc<RegionRepository>,
    pub region_setting_repository: Arc<RegionSettingRepository>,
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
        .route(
            "/api/regions/:region_id/settings",
            get(get_region_settings).post(create_region_setting),
        )
        .route(
            "/api/regions/settings/:setting_id",
            get(get_region_setting_by_id)
                .put(update_region_setting)
                .delete(delete_region_setting),
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
) -> Result<Json<RegionInfoResponse>, ApiError> {
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
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !can_manage {
        return Err(RegionError::InsufficientPermission.into());
    }

    if body.name.is_empty() {
        return Err(RegionError::InvalidRequest.into());
    }

    // Check if group exists
    state
        .group_repository
        .find_by_id(group_id)
        .await?
        .ok_or(GroupError::GroupNotFound)?;

    // Check if region name already exists
    if let Ok(Some(_)) = state.region_repository.find_by_name(&body.name).await {
        return Err(RegionError::RegionNameExists.into());
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

    let mut tx = state.region_repository.get_pool().begin().await?;

    let region_id = state.region_repository.create(&region, &mut tx).await?;

    tx.commit().await?;

    // Get created region
    let created_region = state
        .region_repository
        .find_by_id(region_id)
        .await?
        .ok_or(RegionError::RegionNotFound)?;

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
) -> Result<Json<Vec<RegionInfoResponse>>, ApiError> {
    let current_user_id = token_data.sub;

    // Check if group exists
    state
        .group_repository
        .find_by_id(group_id)
        .await?
        .ok_or(GroupError::GroupNotFound)?;

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
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !can_view {
        return Err(RegionError::InsufficientPermission.into());
    }

    // Get all regions in the group
    let regions = state.region_repository.find_by_group_id(group_id).await?;

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
) -> Result<Json<RegionResponse>, ApiError> {
    let current_user_id = token_data.sub;

    // Use permission service to check if user can access this region
    let can_access = state
        .permission_service
        .can_user_access_region(current_user_id, region_id)
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !can_access {
        return Err(RegionError::InsufficientPermission.into());
    }

    // Get region information
    let region = state
        .region_repository
        .find_by_id(region_id)
        .await?
        .ok_or(RegionError::RegionNotFound)?;

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
    request_body = UpdateRegionRequest,
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
    Json(body): Json<UpdateRegionRequest>,
) -> Result<Json<RegionResponse>, ApiError> {
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
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !can_update {
        return Err(RegionError::InsufficientPermission.into());
    }

    // Get current region information
    let mut region = state
        .region_repository
        .find_by_id(region_id)
        .await?
        .ok_or(RegionError::RegionNotFound)?;

    // Update region information
    if let Some(name) = body.name {
        region.name = name;
    }

    // Update region
    let mut tx = state.region_repository.get_pool().begin().await?;

    state
        .region_repository
        .update(region_id, &region, &mut tx)
        .await?;

    tx.commit().await?;

    // Get updated region information
    let updated_region = state
        .region_repository
        .find_by_id(region_id)
        .await?
        .ok_or(RegionError::RegionNotFound)?;

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
) -> Result<axum::http::StatusCode, ApiError> {
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
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !can_delete {
        return Err(RegionError::InsufficientPermission.into());
    }

    // Check if region exists
    let _ = state
        .region_repository
        .find_by_id(region_id)
        .await?
        .ok_or(RegionError::RegionNotFound)?;

    // Delete region
    let mut tx = state.region_repository.get_pool().begin().await?;

    state.region_repository.delete(region_id, &mut tx).await?;

    tx.commit().await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
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
) -> Result<Json<RegionResponse>, ApiError> {
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
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !can_update {
        return Err(RegionError::InsufficientPermission.into());
    }

    // Get current region information
    let region = state
        .region_repository
        .find_by_id(region_id)
        .await?
        .ok_or(RegionError::RegionNotFound)?;

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

    // Validate environment data
    if light < 0 || !(-50.0..=100.0).contains(&temperature) || !(0.0..=100.0).contains(&humidity) {
        return Err(RegionError::InvalidEnvironmentData.into());
    }

    // Update region environment data
    let mut tx = state.region_repository.get_pool().begin().await?;

    state
        .region_repository
        .update_environment_data(region_id, light, temperature, humidity, &mut tx)
        .await?;

    tx.commit().await?;

    // Get latest region data
    let updated_region = state
        .region_repository
        .find_by_id(region_id)
        .await?
        .ok_or(RegionError::RegionNotFound)?;

    // Use helper function to build response
    let response = build_region_response(&state, &updated_region).await?;

    Ok(Json(response))
}

/// Helper function to build region response, reducing code duplication
async fn build_region_response(
    state: &RegionState,
    region: &Region,
) -> Result<RegionResponse, ApiError> {
    // Get all devices in the region
    let devices = state.device_repository.find_by_region_id(region.id).await?;

    // Get user roles in the region
    let user_roles = state
        .user_region_repository
        .get_region_roles_by_region_id(region.id)
        .await?;

    let devices: Vec<DeviceInfoResponse> = devices
        .into_iter()
        .map(|device| DeviceInfoResponse {
            id: device.id,
            region_id: device.region_id,
            name: device.name,
            device_type: device.device_type.into(),
            location: device.location.clone(),
            status: device.status.clone(),
        })
        .collect();

    let region_settings = state
        .region_setting_repository
        .find_by_region_id(region.id)
        .await?;

    let settings = region_settings
        .into_iter()
        .map(|setting| {
            let data = RegionSettingData {
                light_range: (setting.min_light, setting.max_light),
                temperature_range: (setting.min_temperature, setting.max_temperature),
                humidity_range: (f32::NAN, f32::NAN),
            };

            SettingResponse {
                id: setting.id,
                target_id: setting.region_id,
                data,
                start_time: setting.start,
                end_time: setting.end,
            }
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
        users: user_roles.into_iter().collect(),
        settings,
        devices,
    };

    Ok(region_response)
}

#[utoipa::path(
    post,
    path = "/api/regions/{region_id}/settings",
    tag = "regions",
    params(
        ("region_id" = i32, Path, description = "Region ID")
    ),
    request_body = CreateSettingRequest<RegionSettingData>,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 201, description = "Setting created successfully", body = SettingResponse<RegionSettingData>),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to modify this region"),
        (status = 404, description = "Region not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_region_setting(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<RegionState>,
    Path(region_id): Path<i32>,
    Json(body): Json<CreateSettingRequest<RegionSettingData>>,
) -> Result<Json<SettingResponse<RegionSettingData>>, ApiError> {
    let current_user_id = token_data.sub;

    // Check if region exists
    let _ = state
        .region_repository
        .find_by_id(region_id)
        .await?
        .ok_or(RegionError::RegionNotFound)?;

    // Check if user has permission to modify region settings
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Region,
            region_id,
            Permission::UPDATE,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(RegionError::InsufficientPermission.into());
    }

    // Validate request parameters
    if body.start_time >= body.end_time {
        return Err(SettingError::InvalidTimeRange.into());
    }

    let (min_light, max_light) = body.data.light_range;
    if min_light < 0 || max_light < min_light {
        return Err(SettingError::InvalidLightRange.into());
    }

    let (min_temp, max_temp) = body.data.temperature_range;
    if min_temp < -50.0 || max_temp > 100.0 || max_temp < min_temp {
        return Err(SettingError::InvalidTemperatureRange.into());
    }

    // Create region setting
    let setting = RegionSetting {
        id: 0,
        region_id,
        min_light,
        max_light,
        min_temperature: min_temp,
        max_temperature: max_temp,
        start: body.start_time,
        end: body.end_time,
    };

    let mut tx = state.region_setting_repository.get_pool().begin().await?;
    let setting_id = state
        .region_setting_repository
        .create(&setting, &mut tx)
        .await?;
    tx.commit().await?;

    // Query created setting
    let created_setting = state
        .region_setting_repository
        .find_by_id(setting_id)
        .await?
        .ok_or(SettingError::SettingNotFound)?;

    // Convert to API response format
    let setting_response = SettingResponse {
        id: created_setting.id,
        target_id: created_setting.region_id,
        data: RegionSettingData {
            light_range: (created_setting.min_light, created_setting.max_light),
            temperature_range: (
                created_setting.min_temperature,
                created_setting.max_temperature,
            ),
            humidity_range: (f32::NAN, f32::NAN), // Humidity settings not supported yet
        },
        start_time: created_setting.start,
        end_time: created_setting.end,
    };

    Ok(Json(setting_response))
}

#[utoipa::path(
    get,
    path = "/api/regions/{region_id}/settings",
    tag = "regions",
    params(
        ("region_id" = i32, Path, description = "Region ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Successfully retrieved settings", body = inline(Vec<SettingResponse<RegionSettingData>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to access this region"),
        (status = 404, description = "Region not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_region_settings(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<RegionState>,
    Path(region_id): Path<i32>,
) -> Result<Json<Vec<SettingResponse<RegionSettingData>>>, ApiError> {
    let current_user_id = token_data.sub;

    // Check if region exists
    let _ = state
        .region_repository
        .find_by_id(region_id)
        .await?
        .ok_or(RegionError::RegionNotFound)?;

    // Check if user has permission to view the region
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Region,
            region_id,
            Permission::VIEW,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(RegionError::InsufficientPermission.into());
    }

    // Get all region settings
    let region_settings = state
        .region_setting_repository
        .find_by_region_id(region_id)
        .await?;

    // Convert to API response format
    let setting_responses = region_settings
        .into_iter()
        .map(|setting| SettingResponse {
            id: setting.id,
            target_id: setting.region_id,
            data: RegionSettingData {
                light_range: (setting.min_light, setting.max_light),
                temperature_range: (setting.min_temperature, setting.max_temperature),
                humidity_range: (f32::NAN, f32::NAN), // Humidity settings not supported yet
            },
            start_time: setting.start,
            end_time: setting.end,
        })
        .collect();

    Ok(Json(setting_responses))
}

#[utoipa::path(
    get,
    path = "/api/regions/settings/{setting_id}",
    tag = "regions",
    params(
        ("setting_id" = i32, Path, description = "Setting ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Successfully retrieved setting", body = inline(SettingResponse<RegionSettingData>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to access this setting"),
        (status = 404, description = "Setting not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_region_setting_by_id(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<RegionState>,
    Path(setting_id): Path<i32>,
) -> Result<Json<SettingResponse<RegionSettingData>>, ApiError> {
    let current_user_id = token_data.sub;

    // Get setting information
    let setting = state
        .region_setting_repository
        .find_by_id(setting_id)
        .await?
        .ok_or(SettingError::SettingNotFound)?;

    // Check if user has permission to view the region
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Region,
            setting.region_id,
            Permission::VIEW,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(RegionError::InsufficientPermission.into());
    }

    // Convert to API response format
    let setting_response = SettingResponse {
        id: setting.id,
        target_id: setting.region_id,
        data: RegionSettingData {
            light_range: (setting.min_light, setting.max_light),
            temperature_range: (setting.min_temperature, setting.max_temperature),
            humidity_range: (f32::NAN, f32::NAN), // Humidity settings not supported yet
        },
        start_time: setting.start,
        end_time: setting.end,
    };

    Ok(Json(setting_response))
}

#[utoipa::path(
    put,
    path = "/api/regions/settings/{setting_id}",
    tag = "regions",
    params(
        ("setting_id" = i32, Path, description = "Setting ID")
    ),
    request_body = UpdateSettingRequest<RegionSettingData>,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Setting updated successfully", body = SettingResponse<RegionSettingData>),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to modify this setting"),
        (status = 404, description = "Setting not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_region_setting(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<RegionState>,
    Path(setting_id): Path<i32>,
    Json(body): Json<UpdateSettingRequest<RegionSettingData>>,
) -> Result<Json<SettingResponse<RegionSettingData>>, ApiError> {
    let current_user_id = token_data.sub;

    // Get setting information
    let mut setting = state
        .region_setting_repository
        .find_by_id(setting_id)
        .await?
        .ok_or(SettingError::SettingNotFound)?;

    // Check if user has permission to modify region settings
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Region,
            setting.region_id,
            Permission::UPDATE,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(RegionError::InsufficientPermission.into());
    }

    // Update setting values
    if let Some(data) = &body.data {
        let (min_light, max_light) = data.light_range;
        if min_light < 0 || max_light < min_light {
            return Err(SettingError::InvalidLightRange.into());
        }
        setting.min_light = min_light;
        setting.max_light = max_light;

        let (min_temp, max_temp) = data.temperature_range;
        if min_temp < -50.0 || max_temp > 100.0 || max_temp < min_temp {
            return Err(SettingError::InvalidTemperatureRange.into());
        }
        setting.min_temperature = min_temp;
        setting.max_temperature = max_temp;
    }

    // Update time range
    if let Some(start_time) = body.start_time {
        setting.start = start_time;
    }

    if let Some(end_time) = body.end_time {
        setting.end = end_time;
    }

    if setting.start >= setting.end {
        return Err(SettingError::InvalidTimeRange.into());
    }

    // Update setting
    let mut tx = state.region_setting_repository.get_pool().begin().await?;
    state
        .region_setting_repository
        .update(setting_id, &setting, &mut tx)
        .await?;
    tx.commit().await?;

    // Query updated setting
    let updated_setting = state
        .region_setting_repository
        .find_by_id(setting_id)
        .await?
        .ok_or(SettingError::SettingNotFound)?;

    // Convert to API response format
    let setting_response = SettingResponse {
        id: updated_setting.id,
        target_id: updated_setting.region_id,
        data: RegionSettingData {
            light_range: (updated_setting.min_light, updated_setting.max_light),
            temperature_range: (
                updated_setting.min_temperature,
                updated_setting.max_temperature,
            ),
            humidity_range: (f32::NAN, f32::NAN), // Humidity settings not supported yet
        },
        start_time: updated_setting.start,
        end_time: updated_setting.end,
    };

    Ok(Json(setting_response))
}

#[utoipa::path(
    delete,
    path = "/api/regions/settings/{setting_id}",
    tag = "regions",
    params(
        ("setting_id" = i32, Path, description = "Setting ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 204, description = "Setting deleted successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to delete this setting"),
        (status = 404, description = "Setting not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn delete_region_setting(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<RegionState>,
    Path(setting_id): Path<i32>,
) -> Result<axum::http::StatusCode, ApiError> {
    let current_user_id = token_data.sub;

    // Get setting information
    let setting = state
        .region_setting_repository
        .find_by_id(setting_id)
        .await?
        .ok_or(SettingError::SettingNotFound)?;

    // Check if user has permission to delete region settings
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Region,
            setting.region_id,
            Permission::UPDATE,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(RegionError::InsufficientPermission.into());
    }

    // Delete setting
    let mut tx = state.region_setting_repository.get_pool().begin().await?;
    state
        .region_setting_repository
        .delete(setting_id, &mut tx)
        .await?;
    tx.commit().await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
