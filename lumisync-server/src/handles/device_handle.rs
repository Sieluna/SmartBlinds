use std::sync::Arc;

use anyhow::anyhow;
use axum::extract::{Path, State};
use axum::routing::{get, put};
use axum::{Extension, Json, Router, middleware};
use lumisync_api::models::*;
use serde_json::{Value, json};

use crate::errors::{ApiError, DeviceError, RegionError, SettingError};
use crate::middlewares::{TokenState, auth};
use crate::models::{Device, DeviceSetting};
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
        .route(
            "/api/devices/:device_id/settings",
            get(get_device_settings).post(create_device_setting),
        )
        .route(
            "/api/devices/settings/:setting_id",
            get(get_device_setting_by_id)
                .put(update_device_setting)
                .delete(delete_device_setting),
        )
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
        } else if let Ok(sensor_data) =
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
        status: device.status,
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
    if device.device_type == DeviceType::Window.to_string()
        && (!status.is_object()
            || status.get("position").is_none()
            || !status.get("position").unwrap().is_number())
    {
        return Err(DeviceError::InvalidDeviceStatus.into());
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

#[utoipa::path(
    post,
    path = "/api/devices/{device_id}/settings",
    tag = "devices",
    params(
        ("device_id" = i32, Path, description = "Device ID")
    ),
    request_body = CreateSettingRequest<serde_json::Value>,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 201, description = "Setting created successfully", body = SettingResponse<serde_json::Value>),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to modify this device"),
        (status = 404, description = "Device not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_device_setting(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<DeviceState>,
    Path(device_id): Path<i32>,
    Json(body): Json<CreateSettingRequest<serde_json::Value>>,
) -> Result<Json<SettingResponse<serde_json::Value>>, ApiError> {
    let current_user_id = token_data.sub;

    // Check if device exists
    let device = state
        .device_repository
        .find_by_id(device_id)
        .await?
        .ok_or(DeviceError::DeviceNotFound)?;

    // Check if user has permission to modify the device
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

    // Validate request parameters
    if body.start_time >= body.end_time {
        return Err(SettingError::InvalidTimeRange.into());
    }

    // Validate device setting format
    if device.device_type == DeviceType::Window.to_string() {
        // Check window device settings
        let window_data: Result<WindowSettingData, _> = serde_json::from_value(body.data.clone());
        if window_data.is_err() {
            return Err(SettingError::InvalidDeviceSettingFormat.into());
        }
        let _ = window_data.unwrap();
    } else if device.device_type == DeviceType::Sensor.to_string() {
        // Check sensor device settings
        let sensor_data: Result<SensorSettingData, _> = serde_json::from_value(body.data.clone());
        if sensor_data.is_err() {
            return Err(SettingError::InvalidDeviceSettingFormat.into());
        }
        let _ = sensor_data.unwrap();
    } else {
        return Err(DeviceError::InvalidDeviceStatus.into());
    }

    // Create device setting
    let setting = DeviceSetting {
        id: 0,
        device_id,
        setting: body.data.clone(),
        start: body.start_time,
        end: body.end_time,
    };

    let mut tx = state.device_setting_repository.get_pool().begin().await?;
    let setting_id = state
        .device_setting_repository
        .create(&setting, &mut tx)
        .await?;
    tx.commit().await?;

    // Query created setting
    let created_setting = state
        .device_setting_repository
        .find_by_id(setting_id)
        .await?
        .ok_or(SettingError::SettingNotFound)?;

    // Convert to API response format
    let setting_response = SettingResponse {
        id: created_setting.id,
        target_id: created_setting.device_id,
        data: created_setting.setting,
        start_time: created_setting.start,
        end_time: created_setting.end,
    };

    Ok(Json(setting_response))
}

#[utoipa::path(
    get,
    path = "/api/devices/{device_id}/settings",
    tag = "devices",
    params(
        ("device_id" = i32, Path, description = "Device ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Successfully retrieved settings", body = inline(Vec<SettingResponse<serde_json::Value>>)),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to access this device"),
        (status = 404, description = "Device not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_device_settings(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<DeviceState>,
    Path(device_id): Path<i32>,
) -> Result<Json<Vec<SettingResponse<serde_json::Value>>>, ApiError> {
    let current_user_id = token_data.sub;

    // Check if device exists
    let _device = state
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

    // Convert to API response format
    let mut settings = Vec::new();

    for device_setting in device_settings {
        let setting_response = SettingResponse {
            id: device_setting.id,
            target_id: device_setting.device_id,
            data: device_setting.setting,
            start_time: device_setting.start,
            end_time: device_setting.end,
        };
        settings.push(setting_response);
    }

    Ok(Json(settings))
}

#[utoipa::path(
    get,
    path = "/api/devices/settings/{setting_id}",
    tag = "devices",
    params(
        ("setting_id" = i32, Path, description = "Setting ID")
    ),
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Successfully retrieved setting", body = SettingResponse<serde_json::Value>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to access this setting"),
        (status = 404, description = "Setting not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_device_setting_by_id(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<DeviceState>,
    Path(setting_id): Path<i32>,
) -> Result<Json<SettingResponse<serde_json::Value>>, ApiError> {
    let current_user_id = token_data.sub;

    // Get setting information
    let setting = state
        .device_setting_repository
        .find_by_id(setting_id)
        .await?
        .ok_or(SettingError::SettingNotFound)?;

    // Get device information
    let device = state
        .device_repository
        .find_by_id(setting.device_id)
        .await?
        .ok_or(DeviceError::DeviceNotFound)?;

    // Check if user has permission to view the device setting
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Device,
            device.id,
            Permission::VIEW,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(DeviceError::InsufficientPermission.into());
    }

    // Convert to API response format
    let setting_response = SettingResponse {
        id: setting.id,
        target_id: setting.device_id,
        data: setting.setting,
        start_time: setting.start,
        end_time: setting.end,
    };

    Ok(Json(setting_response))
}

#[utoipa::path(
    put,
    path = "/api/devices/settings/{setting_id}",
    tag = "devices",
    params(
        ("setting_id" = i32, Path, description = "Setting ID")
    ),
    request_body = UpdateSettingRequest<serde_json::Value>,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Setting updated successfully", body = SettingResponse<serde_json::Value>),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "No permission to modify this setting"),
        (status = 404, description = "Setting not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_device_setting(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<DeviceState>,
    Path(setting_id): Path<i32>,
    Json(body): Json<UpdateSettingRequest<serde_json::Value>>,
) -> Result<Json<SettingResponse<serde_json::Value>>, ApiError> {
    let current_user_id = token_data.sub;

    // Get setting information
    let mut setting = state
        .device_setting_repository
        .find_by_id(setting_id)
        .await?
        .ok_or(SettingError::SettingNotFound)?;

    // Get device information
    let device = state
        .device_repository
        .find_by_id(setting.device_id)
        .await?
        .ok_or(DeviceError::DeviceNotFound)?;

    // Check if user has permission to modify the device setting
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Device,
            device.id,
            Permission::DEVICE_UPDATE_SETTINGS,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(DeviceError::InsufficientPermission.into());
    }

    // Update setting data
    if let Some(data) = &body.data {
        // Validate device setting format
        if device.device_type == DeviceType::Window.to_string() {
            // Check window device settings
            let window_data: Result<WindowSettingData, _> = serde_json::from_value(data.clone());
            if window_data.is_err() {
                return Err(SettingError::InvalidDeviceSettingFormat.into());
            }
            let _ = window_data.unwrap();
        } else if device.device_type == DeviceType::Sensor.to_string() {
            // Check sensor device settings
            let sensor_data: Result<SensorSettingData, _> = serde_json::from_value(data.clone());
            if sensor_data.is_err() {
                return Err(SettingError::InvalidDeviceSettingFormat.into());
            }
            let _ = sensor_data.unwrap();
        } else {
            return Err(DeviceError::InvalidDeviceStatus.into());
        }

        setting.setting = data.clone();
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
    let mut tx = state.device_setting_repository.get_pool().begin().await?;
    state
        .device_setting_repository
        .update(setting_id, &setting, &mut tx)
        .await?;
    tx.commit().await?;

    // Query updated setting
    let updated_setting = state
        .device_setting_repository
        .find_by_id(setting_id)
        .await?
        .ok_or(SettingError::SettingNotFound)?;

    // Convert to API response format
    let setting_response = SettingResponse {
        id: updated_setting.id,
        target_id: updated_setting.device_id,
        data: updated_setting.setting,
        start_time: updated_setting.start,
        end_time: updated_setting.end,
    };

    Ok(Json(setting_response))
}

#[utoipa::path(
    delete,
    path = "/api/devices/settings/{setting_id}",
    tag = "devices",
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
pub async fn delete_device_setting(
    Extension(token_data): Extension<TokenClaims>,
    State(state): State<DeviceState>,
    Path(setting_id): Path<i32>,
) -> Result<axum::http::StatusCode, ApiError> {
    let current_user_id = token_data.sub;

    // Get setting information
    let setting = state
        .device_setting_repository
        .find_by_id(setting_id)
        .await?
        .ok_or(SettingError::SettingNotFound)?;

    // Get device information
    let device = state
        .device_repository
        .find_by_id(setting.device_id)
        .await?
        .ok_or(DeviceError::DeviceNotFound)?;

    // Check if user has permission to delete the device setting
    let has_permission = state
        .permission_service
        .check_permission(
            current_user_id,
            ResourceType::Device,
            device.id,
            Permission::DEVICE_UPDATE_SETTINGS,
        )
        .await
        .map_err(|e| anyhow!("Permission check failed: {}", e))?;

    if !has_permission {
        return Err(DeviceError::InsufficientPermission.into());
    }

    // Delete setting
    let mut tx = state.device_setting_repository.get_pool().begin().await?;
    state
        .device_setting_repository
        .delete(setting_id, &mut tx)
        .await?;
    tx.commit().await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
