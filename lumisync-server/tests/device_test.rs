use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt;

use lumisync_api::models::{CreateDeviceRequest, DeviceType, UpdateDeviceRequest};
use lumisync_server::tests::{create_test_group, create_test_region, create_test_user_group};
use serde_json::json;

mod common;
use common::mock_app::MockApp;

#[tokio::test]
async fn test_create_device() {
    let app = MockApp::new().await.with_device_handle();
    let group = create_test_group(app.storage.clone(), "Device Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Device Test Region",
        500,   // light
        22.0,  // temperature
        45.0,  // humidity
        false, // is_public
    )
    .await;

    let request = Request::builder()
        .uri(format!("/api/regions/{}/devices", region.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateDeviceRequest {
                name: "Test Device 1".to_string(),
                device_type: DeviceType::Window,
                location: json!("Bedroom Window"),
                region_id: region.id,
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let device_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(device_response["name"], json!("Test Device 1"));
    assert_eq!(device_response["device_type"], json!("window"));
    assert_eq!(device_response["location"], json!("Bedroom Window"));
    assert_eq!(device_response["region_id"], json!(region.id));

    // Test duplicate name
    let request = Request::builder()
        .uri(format!("/api/regions/{}/devices", region.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateDeviceRequest {
                name: "Test Device 1".to_string(),
                device_type: DeviceType::Window,
                location: json!("Living Room Window"),
                region_id: region.id,
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_get_devices_by_region_id() {
    let app = MockApp::new().await.with_device_handle();
    let group = create_test_group(app.storage.clone(), "Get Devices List Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Get Devices List Region",
        450,   // light
        21.5,  // temperature
        40.0,  // humidity
        false, // is_public
    )
    .await;

    // Create test device
    let create_device_request = Request::builder()
        .uri(format!("/api/regions/{}/devices", region.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateDeviceRequest {
                name: "List Test Device".to_string(),
                device_type: DeviceType::Sensor,
                location: json!("Living Room"),
                region_id: region.id,
            })
            .unwrap(),
        ))
        .unwrap();

    let _ = app
        .router
        .clone()
        .oneshot(create_device_request)
        .await
        .unwrap();

    // Test getting device list
    let request = Request::builder()
        .uri(format!("/api/regions/{}/devices", region.id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let devices: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert!(!devices.is_empty());
    assert!(
        devices
            .iter()
            .any(|d| d["name"] == json!("List Test Device"))
    );

    // Test unauthorized access
    let request = Request::builder()
        .uri(format!("/api/regions/{}/devices", region.id))
        .method(Method::GET)
        .header("Authorization", "Bearer invalid_token")
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_device_by_id() {
    let app = MockApp::new().await.with_device_handle();
    let group = create_test_group(app.storage.clone(), "Get Device Detail Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Get Device Detail Region",
        520,   // light
        23.5,  // temperature
        52.0,  // humidity
        false, // is_public
    )
    .await;

    // Create test device
    let create_device_request = Request::builder()
        .uri(format!("/api/regions/{}/devices", region.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateDeviceRequest {
                name: "Detail Test Device".to_string(),
                device_type: DeviceType::Sensor,
                location: json!("Study"),
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
    let device_id = created_device["id"].as_i64().unwrap();

    // Test getting device details
    let request = Request::builder()
        .uri(format!("/api/devices/{}", device_id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let device_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Fix json path - device info may be directly in the response
    assert_eq!(device_response["id"], json!(device_id));
    assert_eq!(device_response["name"], json!("Detail Test Device"));
    assert_eq!(device_response["location"], json!("Study"));

    // Test getting non-existent device
    let request = Request::builder()
        .uri("/api/devices/9999")
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_device() {
    let app = MockApp::new().await.with_device_handle();
    let group = create_test_group(app.storage.clone(), "Update Device Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Update Device Region",
        480,   // light
        22.8,  // temperature
        47.5,  // humidity
        false, // is_public
    )
    .await;

    // Create test device
    let create_device_request = Request::builder()
        .uri(format!("/api/regions/{}/devices", region.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateDeviceRequest {
                name: "Device To Update".to_string(),
                device_type: DeviceType::Window,
                location: json!("Original Location"),
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
    let device_id = created_device["id"].as_i64().unwrap();

    // Test updating device
    let request = Request::builder()
        .uri(format!("/api/devices/{}", device_id))
        .method(Method::PUT)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&UpdateDeviceRequest {
                name: Some("Updated Device".to_string()),
                location: Some(json!("New Location")),
                device_type: None,
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated_device: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(updated_device["id"], json!(device_id));
    assert_eq!(updated_device["name"], json!("Updated Device"));
    assert_eq!(updated_device["location"], json!("New Location"));
}

#[tokio::test]
async fn test_delete_device() {
    let app = MockApp::new().await.with_device_handle();
    let group = create_test_group(app.storage.clone(), "Delete Device Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Delete Device Region",
        400,   // light
        21.0,  // temperature
        42.5,  // humidity
        false, // is_public
    )
    .await;

    // Create test device
    let create_device_request = Request::builder()
        .uri(format!("/api/regions/{}/devices", region.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateDeviceRequest {
                name: "Device To Delete".to_string(),
                device_type: DeviceType::Sensor,
                location: json!("Hallway"),
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
    let device_id = created_device["id"].as_i64().unwrap();

    // Test deleting device
    let request = Request::builder()
        .uri(format!("/api/devices/{}", device_id))
        .method(Method::DELETE)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify device was deleted
    let request = Request::builder()
        .uri(format!("/api/devices/{}", device_id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_device_status() {
    let app = MockApp::new().await.with_device_handle();
    let group = create_test_group(app.storage.clone(), "Device Status Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Device Status Region",
        550,   // light
        24.0,  // temperature
        55.5,  // humidity
        false, // is_public
    )
    .await;

    // Create test device
    let create_device_request = Request::builder()
        .uri(format!("/api/regions/{}/devices", region.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateDeviceRequest {
                name: "Status Test Device".to_string(),
                device_type: DeviceType::Window,
                location: json!("Kitchen"),
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
    let device_id = created_device["id"].as_i64().unwrap();

    // Test updating device status
    let request = Request::builder()
        .uri(format!("/api/devices/{}/status", device_id))
        .method(Method::PUT)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&json!({
                "position": 80,
            }))
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let command_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(
        command_response["message"]
            .as_str()
            .unwrap()
            .contains("Status Test Device")
    );

    // Get device to verify status was updated
    let request = Request::builder()
        .uri(format!("/api/devices/{}", device_id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let device_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(device_response["status"]["position"], json!(80));
}
