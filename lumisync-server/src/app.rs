use std::sync::Arc;

use axum::routing::{get, post};
use axum::{middleware, Router};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tower_http::cors::CorsLayer;

use crate::configs::{SchemaManager, Settings, Storage};
use crate::handles::*;
use crate::middlewares::{auth, TokenState};
use crate::services::{AuthService, MessageService, TokenService};

pub async fn create_app(settings: &Arc<Settings>) -> Router {
    let (sender, _receiver) = broadcast::channel(100);
    let (client_sender, _client_receiver) = mpsc::channel(100);
    let (edge_sender, edge_receiver) = mpsc::channel(100);
    let storage = Arc::new(
        Storage::new(settings.database.clone(), SchemaManager::default())
            .await
            .unwrap(),
    );

    let message_service = Arc::new(MessageService::new(
        storage.clone(),
        client_sender.clone(),
        edge_sender.clone(),
    ));
    let message_service_clone = message_service.clone();
    tokio::spawn(async move {
        let _ = message_service_clone.start_listening(edge_receiver).await;
    });

    // let analyser_service = Arc::new(AnalyserService::new(&storage, &sender).await.unwrap());
    // analyser_service.start_listener();

    // let actuator_service = ActuatorService::new(settings.embedded.clone())
    //     .map(Arc::new)
    //     .ok();

    let auth_service = Arc::new(AuthService::new());
    let token_service = Arc::new(TokenService::new(settings.auth.clone()));

    let token_state = TokenState {
        token_service: token_service.clone(),
        storage: storage.clone(),
    };

    let user = Router::new()
        .route("/register", post(create_user))
        .route("/authenticate", post(authenticate_user))
        .route(
            "/authorize",
            get(authorize_user)
                .route_layer(middleware::from_fn_with_state(token_state.clone(), auth)),
        )
        .with_state(UserState {
            auth_service: auth_service.clone(),
            token_service: token_service.clone(),
            storage: storage.clone(),
        });

    let settings = Router::new()
        .route("/", get(get_settings).post(create_setting))
        .route("/region/:region_id", get(get_settings_by_region))
        .route_layer(middleware::from_fn_with_state(token_state.clone(), auth))
        .with_state(SettingState {
            storage: storage.clone(),
        });

    let regions = Router::new()
        .route("/", get(get_regions).post(create_region))
        .route_layer(middleware::from_fn_with_state(token_state.clone(), auth))
        .with_state(RegionState {
            storage: storage.clone(),
        });

    let windows = Router::new()
        .route("/", get(get_windows).post(create_window))
        .route(
            "/:window_id",
            get(get_window_owners)
                .put(update_window)
                .delete(delete_window),
        )
        .route("/region/:region_id", get(get_windows_by_region))
        .route_layer(middleware::from_fn_with_state(token_state.clone(), auth))
        .with_state(WindowState {
            storage: storage.clone(),
        });

    let sensors = Router::new()
        .route("/", get(get_sensors))
        .route("/region/:region_id", get(get_sensors_by_region))
        .route("/data/:sensor_id", get(get_sensor_data_in_range))
        .route("/data/sse/:sensor_id", get(get_sensor_data))
        .route_layer(middleware::from_fn_with_state(token_state.clone(), auth))
        .with_state(SensorState {
            storage: storage.clone(),
        });

    let sse = Router::new()
        .route("/", get(sse_handler))
        .route_layer(middleware::from_fn_with_state(token_state.clone(), auth))
        .with_state(SSEState {
            storage: storage.clone(),
            sender: sender.clone(),
        });

    // for debug
    // let control = Router::new()
    //     .route("/:command", get(execute_command))
    //     .with_state(ControlState {
    //         actuator_service: actuator_service.clone(),
    //     });

    Router::new()
        //.nest("/control", control)
        .nest("/users", user)
        .nest("/settings", settings)
        .nest("/regions", regions)
        .nest("/windows", windows)
        .nest("/sensors", sensors)
        .nest("/event", sse)
        .layer(CorsLayer::permissive())
}
