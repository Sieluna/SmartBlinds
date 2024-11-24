#![allow(unused)]

use std::sync::Arc;

use axum::routing::get;
use axum::{Extension, Router, middleware};

use lumisync_api::models::UserRole;
use lumisync_server::configs::{Auth, Database, SchemaManager, Storage};
use lumisync_server::handles::*;
use lumisync_server::middlewares::{TokenState, auth};
use lumisync_server::models::*;
use lumisync_server::repositories::*;
use lumisync_server::services::{AuthService, PermissionService, TokenClaims, TokenService};
use lumisync_server::tests;

pub struct MockApp {
    pub router: Router,
    pub admin: User,
    pub token: String,
    pub storage: Arc<Storage>,
    pub auth_service: Arc<AuthService>,
    pub token_service: Arc<TokenService>,
    pub permission_service: Arc<PermissionService>,
    pub user_repository: Arc<UserRepository>,
    pub group_repository: Arc<GroupRepository>,
    pub region_repository: Arc<RegionRepository>,
    pub region_setting_repository: Arc<RegionSettingRepository>,
    pub user_region_repository: Arc<UserRegionRepository>,
    pub device_repository: Arc<DeviceRepository>,
    pub device_record_repository: Arc<DeviceRecordRepository>,
    pub device_setting_repository: Arc<DeviceSettingRepository>,
}

impl MockApp {
    pub async fn new() -> Self {
        let storage = Arc::new(
            Storage::new(
                Database {
                    migration_path: None,
                    clean_start: true,
                    url: String::from("sqlite::memory:"),
                },
                SchemaManager::default(),
            )
            .await
            .unwrap(),
        );

        let auth_service = Arc::new(AuthService::new());
        let token_service = Arc::new(TokenService::new(Auth {
            secret: String::from("test"),
            expiration: 1000,
        }));
        let permission_service = Arc::new(PermissionService::new(storage.clone()));

        let user_repository = Arc::new(UserRepository::new(storage.clone()));
        let group_repository = Arc::new(GroupRepository::new(storage.clone()));
        let region_repository = Arc::new(RegionRepository::new(storage.clone()));
        let region_setting_repository = Arc::new(RegionSettingRepository::new(storage.clone()));
        let user_region_repository = Arc::new(UserRegionRepository::new(storage.clone()));
        let device_repository = Arc::new(DeviceRepository::new(storage.clone()));
        let device_record_repository = Arc::new(DeviceRecordRepository::new(storage.clone()));
        let device_setting_repository = Arc::new(DeviceSettingRepository::new(storage.clone()));

        let password_hash = auth_service.hash("test").unwrap();

        let admin = tests::create_test_user(
            storage.clone(),
            "admin@test.com",
            &password_hash,
            &UserRole::Admin,
        )
        .await;

        let token_data = token_service.generate_token(admin.clone()).unwrap();

        Self {
            router: Router::new(),
            admin,
            token: token_data.token,
            storage,
            auth_service,
            token_service,
            permission_service,
            user_repository,
            group_repository,
            region_repository,
            region_setting_repository,
            user_region_repository,
            device_repository,
            device_record_repository,
            device_setting_repository,
        }
    }

    pub fn with_auth_middleware(mut self) -> Self {
        let token_state = TokenState {
            token_service: self.token_service.clone(),
            storage: self.storage.clone(),
        };

        self.router = self.router.merge(
            Router::new()
                .route(
                    "/check",
                    get(|Extension(token_data): Extension<TokenClaims>| async move {
                        serde_json::to_string(&token_data).unwrap()
                    }),
                )
                .route_layer(middleware::from_fn_with_state(token_state, auth)),
        );

        self
    }

    pub fn with_auth_handle(mut self) -> Self {
        let auth_state = AuthState {
            auth_service: self.auth_service.clone(),
            token_service: self.token_service.clone(),
            user_repository: self.user_repository.clone(),
            group_repository: self.group_repository.clone(),
        };

        let token_state = TokenState {
            token_service: self.token_service.clone(),
            storage: self.storage.clone(),
        };

        self.router = self.router.merge(auth_router(auth_state, token_state));

        self
    }

    pub fn with_group_handle(mut self) -> Self {
        let group_state = GroupState {
            user_repository: self.user_repository.clone(),
            group_repository: self.group_repository.clone(),
            region_repository: self.region_repository.clone(),
            permission_service: self.permission_service.clone(),
        };

        let token_state = TokenState {
            token_service: self.token_service.clone(),
            storage: self.storage.clone(),
        };

        self.router = self.router.merge(group_router(group_state, token_state));

        self
    }

    pub fn with_region_handle(mut self) -> Self {
        let region_state = RegionState {
            region_repository: self.region_repository.clone(),
            group_repository: self.group_repository.clone(),
            device_repository: self.device_repository.clone(),
            user_region_repository: self.user_region_repository.clone(),
            region_setting_repository: self.region_setting_repository.clone(),
            permission_service: self.permission_service.clone(),
        };

        let token_state = TokenState {
            token_service: self.token_service.clone(),
            storage: self.storage.clone(),
        };

        self.router = self.router.merge(region_router(region_state, token_state));

        self
    }

    pub fn with_device_handle(mut self) -> Self {
        let device_state = DeviceState {
            device_repository: self.device_repository.clone(),
            region_repository: self.region_repository.clone(),
            device_record_repository: self.device_record_repository.clone(),
            device_setting_repository: self.device_setting_repository.clone(),
            permission_service: self.permission_service.clone(),
        };

        let token_state = TokenState {
            token_service: self.token_service.clone(),
            storage: self.storage.clone(),
        };

        self.router = self.router.merge(device_router(device_state, token_state));

        self
    }
}
