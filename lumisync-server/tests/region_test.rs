use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt;

use lumisync_api::models::{CreateRegionRequest, UpdateRegionRequest};
use lumisync_server::tests::{create_test_group, create_test_region, create_test_user_group};
use serde_json::json;

mod common;
use common::mock_app::MockApp;

#[tokio::test]
async fn test_create_region() {
    let app = MockApp::new().await.with_region_handle();
    let group = create_test_group(app.storage.clone(), "Test Region Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;

    let request = Request::builder()
        .uri(format!("/api/groups/{}/regions", group.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateRegionRequest {
                name: "Test Region".to_string(),
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let region_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(region_response["name"], json!("Test Region"));
    assert_eq!(region_response["group_id"], json!(group.id));

    // Test duplicate name
    let request = Request::builder()
        .uri(format!("/api/groups/{}/regions", group.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateRegionRequest {
                name: "Test Region".to_string(),
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_get_regions_by_group_id() {
    let app = MockApp::new().await.with_region_handle();
    let group = create_test_group(app.storage.clone(), "Get Regions List Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    create_test_region(
        app.storage.clone(),
        group.id,
        "Test Region List",
        500,   // light
        22.5,  // temperature
        45.0,  // humidity
        false, // is_public
    )
    .await;

    let request = Request::builder()
        .uri(format!("/api/groups/{}/regions", group.id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let regions: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert!(!regions.is_empty());
    assert!(
        regions
            .iter()
            .any(|r| r["name"] == json!("Test Region List"))
    );

    // Test unauthorized access
    let request = Request::builder()
        .uri(format!("/api/groups/{}/regions", group.id))
        .method(Method::GET)
        .header("Authorization", "Bearer invalid_token")
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_region_by_id() {
    let app = MockApp::new().await.with_region_handle();
    let group = create_test_group(app.storage.clone(), "Region Detail Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Test Region Detail",
        600,   // light
        23.0,  // temperature
        50.0,  // humidity
        false, // is_public
    )
    .await;

    let request = Request::builder()
        .uri(format!("/api/regions/{}", region.id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let region_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(region_response["id"], json!(region.id));
    assert_eq!(region_response["name"], json!("Test Region Detail"));
    assert_eq!(region_response["group_id"], json!(group.id));

    // Test getting non-existent region
    let request = Request::builder()
        .uri("/api/regions/9999")
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_region() {
    let app = MockApp::new().await.with_region_handle();
    let group = create_test_group(app.storage.clone(), "Update Region Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Region To Update",
        400,   // light
        21.0,  // temperature
        55.0,  // humidity
        false, // is_public
    )
    .await;

    let request = Request::builder()
        .uri(format!("/api/regions/{}", region.id))
        .method(Method::PUT)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&UpdateRegionRequest {
                name: Some("Updated Region".to_string()),
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated_region: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(updated_region["id"], json!(region.id));
    assert_eq!(updated_region["name"], json!("Updated Region"));
}

#[tokio::test]
async fn test_delete_region() {
    let app = MockApp::new().await.with_region_handle();
    let group = create_test_group(app.storage.clone(), "Delete Region Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Region To Delete",
        300,   // light
        20.5,  // temperature
        40.0,  // humidity
        false, // is_public
    )
    .await;

    let request = Request::builder()
        .uri(format!("/api/regions/{}", region.id))
        .method(Method::DELETE)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify region was deleted
    let request = Request::builder()
        .uri(format!("/api/regions/{}", region.id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_region_environment() {
    let app = MockApp::new().await.with_region_handle();
    let group = create_test_group(app.storage.clone(), "Environment Update Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Environment Test Region",
        450,   // light
        22.0,  // temperature
        48.0,  // humidity
        false, // is_public
    )
    .await;

    let request = Request::builder()
        .uri(format!("/api/regions/{}/environment", region.id))
        .method(Method::PUT)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&json!({
                "light": 800,
                "temperature": 25.5,
                "humidity": 60.0
            }))
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated_region: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(updated_region["id"], json!(region.id));
    assert_eq!(updated_region["light"], json!(800));
    assert_eq!(updated_region["temperature"], json!(25.5));
    assert_eq!(updated_region["humidity"], json!(60.0));
}
