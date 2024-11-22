use std::sync::Arc;

use axum::response::Html;
use axum::routing::get;
use axum::{Json, Router};
use lumisync_api::models::*;
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::configs::{SchemaManager, Settings, Storage};
use crate::handles::*;
use crate::middlewares::TokenState;
use crate::repositories::*;
use crate::services::{
    AuthService, MessageService, MessageServiceConfig, PermissionService, TokenService,
};

fn openapi() -> Router {
    use utoipa::OpenApi;

    #[derive(OpenApi)]
    #[openapi(
        paths(
            // auth
            register,
            login,
            refresh_token,
            get_current_user,
            // group
            create_group,
            get_user_groups,
            get_group_by_id,
            update_group,
            delete_group,
            get_group_users,
            // region
            create_region,
            get_regions_by_group_id,
            get_region_by_id,
            update_region,
            delete_region,
            update_region_environment,
            create_region_setting,
            get_region_settings,
            get_region_setting_by_id,
            update_region_setting,
            delete_region_setting,
            // device
            create_device,
            get_devices_by_region_id,
            get_device_by_id,
            update_device,
            delete_device,
            update_device_status,
            create_device_setting,
            get_device_settings,
            get_device_setting_by_id,
            update_device_setting,
            delete_device_setting,
        ),
        components(
            schemas(
                // auth
                UserRole,
                RegionRole,
                LoginRequest,
                RegisterRequest,
                UserInfoResponse,
                UserResponse,
                TokenResponse,
                // group
                CreateGroupRequest,
                GroupResponse,
                // region
                RegionRole,
                CreateRegionRequest,
                UpdateRegionRequest,
                RegionInfoResponse,
                RegionResponse,
                // device
                DeviceType,
                CreateDeviceRequest,
                UpdateDeviceRequest,
                DeviceRecordResponse,
                DeviceSettingResponse,
                DeviceInfoResponse,
                DeviceResponse,
                // settings
                WindowSettingData,
                SensorSettingData,
                RegionSettingData,
                CreateSettingRequest<WindowSettingData>,
                CreateSettingRequest<SensorSettingData>,
                CreateSettingRequest<RegionSettingData>,
                UpdateSettingRequest<WindowSettingData>,
                UpdateSettingRequest<SensorSettingData>,
                UpdateSettingRequest<RegionSettingData>,
                SettingResponse<WindowSettingData>,
                SettingResponse<SensorSettingData>,
                SettingResponse<RegionSettingData>,
            )
        ),
        tags(
            (name = "auth", description = "Authentication related endpoints"),
            (name = "group", description = "Group related endpoints"),
            (name = "region", description = "Region related endpoints"),
            (name = "device", description = "Device related endpoints"),
        )
    )]
    struct ApiDoc;

    const OPENAPI_ENDPOINT: &str = "/openapi.json";

    Router::new()
        .route(OPENAPI_ENDPOINT, get(||async { Json(ApiDoc::openapi()) }))
        .route("/", get(|| async {
            Html(format!(
                r#"
                <!doctype html>
                <html>
                <head>
                    <meta charset="utf-8">
                    <script type="module" src="https://unpkg.com/rapidoc/dist/rapidoc-min.js"></script>
                </head>
                <body>
                    <rapi-doc
                        spec-url="{}"
                        theme="light"
                        show-header="false"
                    ></rapi-doc>
                </body>
                </html>
                "#,
                OPENAPI_ENDPOINT
            ))
        }))
}

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": time::OffsetDateTime::now_utc().to_string()
    }))
}

pub async fn create_app(settings: &Arc<Settings>) -> Router {
    let storage = Arc::new(
        Storage::new(settings.database.clone(), SchemaManager::default())
            .await
            .unwrap(),
    );

    let mut message_service = MessageService::new(storage.clone());
    let message_config = MessageServiceConfig {
        websocket_addr: "127.0.0.1:8080".parse().unwrap(),
        tcp_addr: "127.0.0.1:9000".parse().unwrap(),
        enable_websocket: true,
        enable_tcp: true,
    };

    message_service.init_protocols(message_config).unwrap();
    message_service.start().await.unwrap();

    let user_repository = Arc::new(UserRepository::new(storage.clone()));
    let group_repository = Arc::new(GroupRepository::new(storage.clone()));
    let region_repository = Arc::new(RegionRepository::new(storage.clone()));
    let user_region_repository = Arc::new(UserRegionRepository::new(storage.clone()));
    let device_repository = Arc::new(DeviceRepository::new(storage.clone()));
    let device_record_repository = Arc::new(DeviceRecordRepository::new(storage.clone()));
    let device_setting_repository = Arc::new(DeviceSettingRepository::new(storage.clone()));
    let region_setting_repository = Arc::new(RegionSettingRepository::new(storage.clone()));
    let _ = Arc::new(EventRepository::new(storage.clone()));

    let auth_service = Arc::new(AuthService::new());
    let token_service = Arc::new(TokenService::new(settings.auth.clone()));
    let permission_service = Arc::new(PermissionService::new(storage.clone()));

    let auth_state = AuthState {
        auth_service: auth_service.clone(),
        token_service: token_service.clone(),
        user_repository: user_repository.clone(),
        group_repository: group_repository.clone(),
    };

    let token_state = TokenState {
        token_service: token_service.clone(),
        storage: storage.clone(),
    };

    let group_state = GroupState {
        user_repository: user_repository.clone(),
        group_repository: group_repository.clone(),
        region_repository: region_repository.clone(),
        permission_service: permission_service.clone(),
    };

    let region_state = RegionState {
        user_region_repository: user_region_repository.clone(),
        region_repository: region_repository.clone(),
        group_repository: group_repository.clone(),
        device_repository: device_repository.clone(),
        permission_service: permission_service.clone(),
        region_setting_repository: region_setting_repository.clone(),
    };

    let device_state = DeviceState {
        device_repository: device_repository.clone(),
        region_repository: region_repository.clone(),
        permission_service: permission_service.clone(),
        device_setting_repository: device_setting_repository.clone(),
        device_record_repository: device_record_repository.clone(),
    };

    Router::new()
        .merge(auth_router(auth_state.clone(), token_state.clone()))
        .merge(group_router(group_state.clone(), token_state.clone()))
        .merge(region_router(region_state.clone(), token_state.clone()))
        .merge(device_router(device_state.clone(), token_state.clone()))
        .merge(openapi())
        .route("/health", get(health_check))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}
