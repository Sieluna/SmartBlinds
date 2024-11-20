use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt;

use lumisync_api::models::*;
use lumisync_server::tests::*;
use serde_json::json;
use time::OffsetDateTime;

mod common;
use common::mock_app::MockApp;

#[tokio::test]
async fn test_device_setting_create() {
    let app = MockApp::new().await.with_device_handle();
    let group = create_test_group(app.storage.clone(), "Device Setting Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Device Setting Test Region",
        500,
        22.0,
        45.0,
        false,
    )
    .await;

    let create_device_request = Request::builder()
        .uri(format!("/api/regions/{}/devices", region.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateDeviceRequest {
                name: "API Test Device for Settings".to_string(),
                device_type: DeviceType::Window,
                location: json!("Living Room"),
                region_id: region.id,
            })
            .unwrap(),
        ))
        .unwrap();

    let create_response = app
        .router
        .clone()
        .oneshot(create_device_request)
        .await
        .unwrap();
    let create_body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_device: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    let device_id = created_device["id"].as_i64().unwrap() as i32;

    let now = OffsetDateTime::now_utc();
    let tomorrow = now + time::Duration::days(1);

    let window_setting = json!({
        "position_range": [0, 100],
        "auto_adjust": true
    });

    let create_setting_request = Request::builder()
        .uri(format!("/api/devices/{}/settings", device_id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateSettingRequest {
                target_id: device_id,
                data: window_setting,
                start_time: now,
                end_time: tomorrow,
            })
            .unwrap(),
        ))
        .unwrap();

    let create_setting_response = app
        .router
        .clone()
        .oneshot(create_setting_request)
        .await
        .unwrap();

    assert_eq!(create_setting_response.status(), StatusCode::OK);

    let create_setting_body = axum::body::to_bytes(create_setting_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_setting: serde_json::Value = serde_json::from_slice(&create_setting_body).unwrap();

    assert_eq!(created_setting["target_id"], device_id);
    assert_eq!(created_setting["data"]["position_range"][0], 0);
    assert_eq!(created_setting["data"]["position_range"][1], 100);
    assert_eq!(created_setting["data"]["auto_adjust"], true);
}

#[tokio::test]
async fn test_device_setting_get_list() {
    let app = MockApp::new().await.with_device_handle();
    let group = create_test_group(app.storage.clone(), "Device Setting List Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Device Setting List Test",
        500,   // light
        22.0,  // temperature
        45.0,  // humidity
        false, // is_public
    )
    .await;

    let device = create_test_device(
        app.storage.clone(),
        region.id,
        "Test Device for List",
        &DeviceType::Window,
        json!({"position": 50}),
    )
    .await;

    let now = OffsetDateTime::now_utc();
    let tomorrow = now + time::Duration::days(1);
    let next_week = now + time::Duration::days(7);

    let setting1 = create_test_device_setting(
        app.storage.clone(),
        device.id,
        json!({
            "position_range": [0, 100],
            "auto_adjust": true
        }),
        now,
        tomorrow,
    )
    .await;

    let setting2 = create_test_device_setting(
        app.storage.clone(),
        device.id,
        json!({
            "position_range": [10, 90],
            "auto_adjust": false
        }),
        tomorrow + time::Duration::hours(1),
        next_week,
    )
    .await;

    let get_settings_request = Request::builder()
        .uri(format!("/api/devices/{}/settings", device.id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let get_settings_response = app
        .router
        .clone()
        .oneshot(get_settings_request)
        .await
        .unwrap();

    assert_eq!(get_settings_response.status(), StatusCode::OK);

    let get_settings_body = axum::body::to_bytes(get_settings_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let settings: Vec<serde_json::Value> = serde_json::from_slice(&get_settings_body).unwrap();

    assert!(settings.len() >= 2);
    assert!(settings.iter().any(|s| s["id"] == json!(setting1.id)));
    assert!(settings.iter().any(|s| s["id"] == json!(setting2.id)));
}

#[tokio::test]
async fn test_device_setting_get_by_id() {
    let app = MockApp::new().await.with_device_handle();
    let group = create_test_group(app.storage.clone(), "Device Setting Get Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Device Setting Get Test",
        500,   // light
        22.0,  // temperature
        45.0,  // humidity
        false, // is_public
    )
    .await;

    let device = create_test_device(
        app.storage.clone(),
        region.id,
        "Test Device for Get",
        &DeviceType::Window,
        json!({"position": 50}),
    )
    .await;

    let now = OffsetDateTime::now_utc();
    let tomorrow = now + time::Duration::days(1);

    let setting = create_test_device_setting(
        app.storage.clone(),
        device.id,
        json!({
            "position_range": [0, 100],
            "auto_adjust": true
        }),
        now,
        tomorrow,
    )
    .await;

    let get_setting_request = Request::builder()
        .uri(format!("/api/devices/settings/{}", setting.id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let get_setting_response = app
        .router
        .clone()
        .oneshot(get_setting_request)
        .await
        .unwrap();

    assert_eq!(get_setting_response.status(), StatusCode::OK);

    let get_setting_body = axum::body::to_bytes(get_setting_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let setting_response: serde_json::Value = serde_json::from_slice(&get_setting_body).unwrap();

    assert_eq!(setting_response["id"], json!(setting.id));
    assert_eq!(setting_response["target_id"], json!(device.id));
}

#[tokio::test]
async fn test_device_setting_update() {
    let app = MockApp::new().await.with_device_handle();
    let group = create_test_group(app.storage.clone(), "Device Setting Update Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Device Setting Update Test",
        500,   // light
        22.0,  // temperature
        45.0,  // humidity
        false, // is_public
    )
    .await;

    let device = create_test_device(
        app.storage.clone(),
        region.id,
        "Test Device for Update",
        &DeviceType::Window,
        json!({"position": 50}),
    )
    .await;

    let now = OffsetDateTime::now_utc();
    let tomorrow = now + time::Duration::days(1);

    let setting = create_test_device_setting(
        app.storage.clone(),
        device.id,
        json!({
            "position_range": [0, 100],
            "auto_adjust": true
        }),
        now,
        tomorrow,
    )
    .await;

    let updated_window_setting = json!({
        "position_range": [10, 80],
        "auto_adjust": false
    });

    let update_setting_request = Request::builder()
        .uri(format!("/api/devices/settings/{}", setting.id))
        .method(Method::PUT)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&UpdateSettingRequest {
                data: Some(updated_window_setting),
                start_time: None,
                end_time: None,
            })
            .unwrap(),
        ))
        .unwrap();

    let update_setting_response = app
        .router
        .clone()
        .oneshot(update_setting_request)
        .await
        .unwrap();

    assert_eq!(update_setting_response.status(), StatusCode::OK);

    let update_setting_body = axum::body::to_bytes(update_setting_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated_setting: serde_json::Value = serde_json::from_slice(&update_setting_body).unwrap();

    assert_eq!(updated_setting["data"]["auto_adjust"], json!(false));
    assert_eq!(updated_setting["data"]["position_range"][0], json!(10));
    assert_eq!(updated_setting["data"]["position_range"][1], json!(80));
}

#[tokio::test]
async fn test_device_setting_delete() {
    let app = MockApp::new().await.with_device_handle();
    let group = create_test_group(app.storage.clone(), "Device Setting Delete Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Device Setting Delete Test",
        500,   // light
        22.0,  // temperature
        45.0,  // humidity
        false, // is_public
    )
    .await;

    let device = create_test_device(
        app.storage.clone(),
        region.id,
        "Test Device for Delete",
        &DeviceType::Window,
        json!({"position": 50}),
    )
    .await;
    let now = OffsetDateTime::now_utc();
    let tomorrow = now + time::Duration::days(1);

    let setting = create_test_device_setting(
        app.storage.clone(),
        device.id,
        json!({
            "position_range": [0, 100],
            "auto_adjust": true
        }),
        now,
        tomorrow,
    )
    .await;

    let delete_setting_request = Request::builder()
        .uri(format!("/api/devices/settings/{}", setting.id))
        .method(Method::DELETE)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let delete_setting_response = app
        .router
        .clone()
        .oneshot(delete_setting_request)
        .await
        .unwrap();

    assert_eq!(delete_setting_response.status(), StatusCode::NO_CONTENT);

    let verify_request = Request::builder()
        .uri(format!("/api/devices/settings/{}", setting.id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let verify_response = app.router.clone().oneshot(verify_request).await.unwrap();

    assert_eq!(verify_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_device_setting_validation() {
    let app = MockApp::new().await.with_device_handle();
    let group = create_test_group(app.storage.clone(), "Device Setting Validation Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Device Setting Validation Region",
        500,   // light
        22.0,  // temperature
        45.0,  // humidity
        false, // is_public
    )
    .await;

    let device = create_test_device(
        app.storage.clone(),
        region.id,
        "Validation Test Device",
        &DeviceType::Window,
        json!({"position": 50}),
    )
    .await;

    let now = OffsetDateTime::now_utc();

    let window_setting = json!({
        "position_range": [0, 100],
        "auto_adjust": true
    });

    let invalid_time_request = Request::builder()
        .uri(format!("/api/devices/{}/settings", device.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateSettingRequest {
                target_id: device.id,
                data: window_setting,
                start_time: now,
                end_time: now,
            })
            .unwrap(),
        ))
        .unwrap();

    let invalid_time_response = app
        .router
        .clone()
        .oneshot(invalid_time_request)
        .await
        .unwrap();

    assert_eq!(invalid_time_response.status(), StatusCode::BAD_REQUEST);

    let invalid_format_request = Request::builder()
        .uri(format!("/api/devices/{}/settings", device.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateSettingRequest {
                target_id: device.id,
                data: json!({ "invalid_field": "value" }),
                start_time: now,
                end_time: now + time::Duration::days(1),
            })
            .unwrap(),
        ))
        .unwrap();

    let invalid_format_response = app
        .router
        .clone()
        .oneshot(invalid_format_request)
        .await
        .unwrap();

    assert_eq!(invalid_format_response.status(), StatusCode::BAD_REQUEST);
}
