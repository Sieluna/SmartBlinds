use std::sync::Arc;

use anyhow::anyhow;
use axum::extract::{Path, State};
use axum::routing::{get, put};
use axum::{middleware, Extension, Json, Router};
use lumisync_api::models::*;
use serde_json::{json, Value};

use crate::errors::{ApiError, DeviceError, RegionError};
use crate::middlewares::{auth, TokenState};
use crate::models::Device;
use crate::repositories::{
    DeviceRecordRepository, DeviceRepository, DeviceSettingRepository, RegionRepository,
};
use crate::services::{Permission, PermissionService, ResourceType, TokenClaims};

#[derive(Clone)]
pub struct DeviceState {
    pub device_repository: Arc<DeviceRepository>,
    pub device_record_repository: Arc<DeviceRecordRepository>,
    pub device_setting_repository: Arc<DeviceSettingRepository>,
    pub region_repository: Arc<RegionRepository>,
    pub permission_service: Arc<PermissionService>,
}

pub fn device_router(device_state: DeviceState, token_state: TokenState) -> Router {
    Router::new()
        .route(
            "/api/regions/:region_id/devices",
            get(get_devices_by_region_id).post(create_device),
        )
        .route(
            "/api/devices/:device_id",
            get(get_device_by_id)
                .put(update_device)
                .delete(delete_device),
        )
        .route("/api/devices/:device_id/status", put(update_device_status))
        .route_layer(middleware::from_fn_with_state(token_state, auth))
        .with_state(device_state)
}

#[utoipa::path(
    post,
    path = "/api/regions/{region_id}/devices",
    tag = "device",
    params(
        ("region_id" = i32, Path, description = "Region ID")
    ),
    request_body = CreateDeviceRequest,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 201, description = "Device created successfully", body = DeviceInfoResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to create device in this region"),
        (status = 404, description = "Region not found"),
        (status = 409, description = "Device name already exists"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_device(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<DeviceState>,
    Path(region_id): Path<i32>,
    Json(body): Json<CreateDeviceRequest>,
) -> Result<Json<DeviceInfoResponse>, ApiError> {
    // Validate request
    if body.name.is_empty() {
        return Err(DeviceError::InvalidRequest.into());
    }

    let current_user_id = token_data.sub;

    // Check if user has permission to manage devices in the region
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Region,
            region_id,
            Permission::REGION_MANAGE_DEVICES,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(DeviceError::InsufficientPermission.into());
    }

    // Check if region exists
    let _ = state
        .region_repository
        .find_by_id(region_id)
        .await?
        .ok_or(RegionError::RegionNotFound)?;

    // Check if device name already exists
    if let Ok(Some(_)) = state.device_repository.find_by_name(&body.name).await {
        return Err(DeviceError::DeviceNameExists.into());
    }

    // Create device
    let device = Device {
        id: 0,
        region_id,
        name: body.name.clone(),
        device_type: body.device_type.to_string(),
        location: body.location.clone(),
        status: json!({}), // Initial status is empty
    };

    let pool = state.device_repository.get_pool();
    let mut tx = pool.begin().await?;

    let device_id = state.device_repository.create(&device, &mut tx).await?;

    tx.commit().await?;

    // Get created device
    let created_device = state
        .device_repository
        .find_by_id(device_id)
        .await?
        .ok_or(DeviceError::DeviceNotFound)?;

    let device_response = DeviceInfoResponse {
        id: created_device.id,
        region_id: created_device.region_id,
        name: created_device.name,
        device_type: created_device.device_type.into(),
        location: created_device.location,
        status: created_device.status,
    };

    Ok(Json(device_response))
}

#[utoipa::path(
    get,
    path = "/api/regions/{region_id}/devices",
    tag = "device",
    params(
        ("region_id" = i32, Path, description = "Region ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Successfully retrieved device list", body = Vec<DeviceInfoResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to access devices in this region"),
        (status = 404, description = "Region not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_devices_by_region_id(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<DeviceState>,
    Path(region_id): Path<i32>,
) -> Result<Json<Vec<DeviceInfoResponse>>, ApiError> {
    let current_user_id = token_data.sub;

    // Check if region exists
    state
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
        return Err(DeviceError::InsufficientPermission.into());
    }

    // Get all devices in the region
    let devices = state.device_repository.find_by_region_id(region_id).await?;

    let device_responses: Vec<DeviceInfoResponse> = devices
        .into_iter()
        .map(|device| DeviceInfoResponse {
            id: device.id,
            region_id: device.region_id,
            name: device.name,
            device_type: device.device_type.into(),
            location: device.location,
            status: device.status,
        })
        .collect();

    Ok(Json(device_responses))
}

#[utoipa::path(
    get,
    path = "/api/devices/{device_id}",
    tag = "device",
    params(
        ("device_id" = i32, Path, description = "Device ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Successfully retrieved device details", body = DeviceResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to access this device"),
        (status = 404, description = "Device not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_device_by_id(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<DeviceState>,
    Path(device_id): Path<i32>,
) -> Result<Json<DeviceResponse>, ApiError> {
    let current_user_id = token_data.sub;

    // Get device information
    let device = state
        .device_repository
        .find_by_id(device_id)
        .await?
        .ok_or(DeviceError::DeviceNotFound)?;

    // Check if user has permission to view the device
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Device,
            device_id,
            Permission::VIEW,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(DeviceError::InsufficientPermission.into());
    }

    // Get device settings
    let device_settings = state
        .device_setting_repository
        .find_by_device_id(device_id)
        .await?;

    let mut settings: Vec<DeviceSettingUnion> = Vec::new();
    for device_setting in device_settings {
        if device.device_type == DeviceType::Window.to_string() {
            if let Ok(window_data) =
                serde_json::from_value::<WindowSettingData>(device_setting.setting.clone())
            {
                let setting_response = SettingResponse {
                    id: device_setting.id,
                    target_id: device_setting.device_id,
                    data: window_data,
                    start_time: device_setting.start,
                    end_time: device_setting.end,
                };
                settings.push(DeviceSettingUnion::Window(setting_response));
            } else {
                return Err(DeviceError::InvalidDeviceSetting.into());
            }
        } else {
            if let Ok(sensor_data) =
                serde_json::from_value::<SensorSettingData>(device_setting.setting.clone())
            {
                let setting_response = SettingResponse {
                    id: device_setting.id,
                    target_id: device_setting.device_id,
                    data: sensor_data,
                    start_time: device_setting.start,
                    end_time: device_setting.end,
                };
                settings.push(DeviceSettingUnion::Sensor(setting_response));
            } else {
                return Err(DeviceError::InvalidDeviceSetting.into());
            }
        }
    }

    // Get device records
    let records = state
        .device_record_repository
        .find_by_device_id(device_id)
        .await?
        .into_iter()
        .map(|record| DeviceRecordResponse {
            id: record.id,
            device_id: record.device_id,
            data: record.data,
            time: record.time,
        })
        .collect();

    let device_info = DeviceInfoResponse {
        id: device.id,
        region_id: device.region_id,
        name: device.name,
        device_type: device.device_type.into(),
        location: device.location.clone(),
        status: device.status.clone(),
    };

    let device_response = DeviceResponse {
        info: device_info,
        settings,
        records,
    };

    Ok(Json(device_response))
}

#[utoipa::path(
    put,
    path = "/api/devices/{device_id}",
    tag = "device",
    params(
        ("device_id" = i32, Path, description = "Device ID")
    ),
    request_body = UpdateDeviceRequest,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Device updated successfully", body = DeviceInfoResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to modify this device"),
        (status = 404, description = "Device not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_device(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<DeviceState>,
    Path(device_id): Path<i32>,
    Json(body): Json<UpdateDeviceRequest>,
) -> Result<Json<DeviceInfoResponse>, ApiError> {
    let current_user_id = token_data.sub;

    // Get device information
    let mut device = state
        .device_repository
        .find_by_id(device_id)
        .await?
        .ok_or(DeviceError::DeviceNotFound)?;

    // Check if user has permission to update the device
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Device,
            device_id,
            Permission::DEVICE_UPDATE_SETTINGS,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(DeviceError::InsufficientPermission.into());
    }

    // Update device information
    if let Some(name) = body.name {
        if name.is_empty() {
            return Err(DeviceError::InvalidRequest.into());
        }

        // Check if the new name already exists for another device
        if let Ok(Some(existing_device)) = state.device_repository.find_by_name(&name).await {
            if existing_device.id != device_id {
                return Err(DeviceError::DeviceNameExists.into());
            }
        }

        device.name = name;
    }

    if let Some(location) = body.location {
        device.location = location;
    }

    let mut tx = state.device_repository.get_pool().begin().await?;

    state
        .device_repository
        .update(device_id, &device, &mut tx)
        .await?;

    tx.commit().await?;

    // Get updated device information
    let updated_device = state
        .device_repository
        .find_by_id(device_id)
        .await?
        .ok_or(DeviceError::DeviceNotFound)?;

    let device_response = DeviceInfoResponse {
        id: updated_device.id,
        region_id: updated_device.region_id,
        name: updated_device.name,
        device_type: updated_device.device_type.into(),
        location: updated_device.location,
        status: updated_device.status,
    };

    Ok(Json(device_response))
}

#[utoipa::path(
    delete,
    path = "/api/devices/{device_id}",
    tag = "device",
    params(
        ("device_id" = i32, Path, description = "Device ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 204, description = "Device deleted successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to delete this device"),
        (status = 404, description = "Device not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn delete_device(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<DeviceState>,
    Path(device_id): Path<i32>,
) -> Result<axum::http::StatusCode, ApiError> {
    let current_user_id = token_data.sub;

    // Check if device exists
    let _ = state
        .device_repository
        .find_by_id(device_id)
        .await?
        .ok_or(DeviceError::DeviceNotFound)?;

    // Check if user has permission to delete the device
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Device,
            device_id,
            Permission::DELETE,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(DeviceError::InsufficientPermission.into());
    }

    let mut tx = state.device_repository.get_pool().begin().await?;

    state.device_repository.delete(device_id, &mut tx).await?;

    tx.commit().await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/api/devices/{device_id}/status",
    tag = "device",
    params(
        ("device_id" = i32, Path, description = "Device ID")
    ),
    request_body = Value,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Device status updated successfully", body = CommandResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to control this device"),
        (status = 404, description = "Device not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_device_status(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<DeviceState>,
    Path(device_id): Path<i32>,
    Json(status): Json<Value>,
) -> Result<Json<CommandResponse>, ApiError> {
    let current_user_id = token_data.sub;

    // Check if device exists
    let device = state
        .device_repository
        .find_by_id(device_id)
        .await?
        .ok_or(DeviceError::DeviceNotFound)?;

    // Validate status data format based on device type
    if device.device_type == DeviceType::Window.to_string() {
        if !status.is_object()
            || !status.get("position").is_some()
            || !status.get("position").unwrap().is_number()
        {
            return Err(DeviceError::InvalidDeviceStatus.into());
        }
    }

    // Check if user has permission to control the device
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Device,
            device_id,
            Permission::DEVICE_CONTROL,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(DeviceError::InsufficientPermission.into());
    }

    let mut tx = state.device_repository.get_pool().begin().await?;

    state
        .device_repository
        .update_status(device_id, &status, &mut tx)
        .await?;

    tx.commit().await?;

    let command_response = CommandResponse {
        message: format!("Device {} status updated", device.name),
    };

    Ok(Json(command_response))
}
