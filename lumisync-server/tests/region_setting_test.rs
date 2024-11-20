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
async fn test_region_setting_create() {
    let app = MockApp::new().await.with_region_handle();
    let group = create_test_group(app.storage.clone(), "Region Setting Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Region Setting Test Region",
        500,
        22.0,
        45.0,
        false,
    )
    .await;

    let now = OffsetDateTime::now_utc();
    let tomorrow = now + time::Duration::days(1);

    let region_setting = RegionSettingData {
        light_range: (200, 800),
        temperature_range: (18.0, 26.0),
        humidity_range: (40.0, 60.0),
    };

    let create_setting_request = Request::builder()
        .uri(format!("/api/regions/{}/settings", region.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateSettingRequest {
                target_id: region.id,
                data: region_setting,
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

    assert_eq!(created_setting["target_id"], region.id);
    assert_eq!(created_setting["data"]["light_range"][0], 200);
    assert_eq!(created_setting["data"]["light_range"][1], 800);
    assert_eq!(created_setting["data"]["temperature_range"][0], 18.0);
    assert_eq!(created_setting["data"]["temperature_range"][1], 26.0);
}

#[tokio::test]
async fn test_region_setting_get_list() {
    let app = MockApp::new().await.with_region_handle();
    let group = create_test_group(app.storage.clone(), "Region Setting List Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Region Setting List Test",
        500,
        22.0,
        45.0,
        false,
    )
    .await;

    let now = OffsetDateTime::now_utc();
    let tomorrow = now + time::Duration::days(1);

    let setting1 = create_test_region_setting(
        app.storage.clone(),
        region.id,
        200,
        800,
        18.0,
        26.0,
        now,
        tomorrow,
    )
    .await;

    let setting2 = create_test_region_setting(
        app.storage.clone(),
        region.id,
        300,
        700,
        19.0,
        25.0,
        now,
        tomorrow + time::Duration::days(1),
    )
    .await;

    let get_settings_request = Request::builder()
        .uri(format!("/api/regions/{}/settings", region.id))
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
async fn test_region_setting_get_by_id() {
    let app = MockApp::new().await.with_region_handle();
    let group = create_test_group(app.storage.clone(), "Region Setting Get Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Region Setting Get Test",
        500,
        22.0,
        45.0,
        false,
    )
    .await;

    let now = OffsetDateTime::now_utc();
    let tomorrow = now + time::Duration::days(1);

    let setting = create_test_region_setting(
        app.storage.clone(),
        region.id,
        200,
        800,
        18.0,
        26.0,
        now,
        tomorrow,
    )
    .await;

    let get_setting_request = Request::builder()
        .uri(format!("/api/regions/settings/{}", setting.id))
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
    assert_eq!(setting_response["target_id"], json!(region.id));
}

#[tokio::test]
async fn test_region_setting_update() {
    let app = MockApp::new().await.with_region_handle();
    let group = create_test_group(app.storage.clone(), "Region Setting Update Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Region Setting Update Test",
        500,
        22.0,
        45.0,
        false,
    )
    .await;

    let now = OffsetDateTime::now_utc();
    let tomorrow = now + time::Duration::days(1);

    let setting = create_test_region_setting(
        app.storage.clone(),
        region.id,
        200,
        800,
        18.0,
        26.0,
        now,
        tomorrow,
    )
    .await;

    let updated_region_setting = RegionSettingData {
        light_range: (300, 700),
        temperature_range: (20.0, 24.0),
        humidity_range: (45.0, 55.0),
    };

    let update_setting_request = Request::builder()
        .uri(format!("/api/regions/settings/{}", setting.id))
        .method(Method::PUT)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&UpdateSettingRequest {
                data: Some(updated_region_setting),
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

    assert_eq!(updated_setting["data"]["light_range"][0], json!(300));
    assert_eq!(updated_setting["data"]["light_range"][1], json!(700));
    assert_eq!(updated_setting["data"]["temperature_range"][0], json!(20.0));
    assert_eq!(updated_setting["data"]["temperature_range"][1], json!(24.0));
}

#[tokio::test]
async fn test_region_setting_delete() {
    let app = MockApp::new().await.with_region_handle();
    let group = create_test_group(app.storage.clone(), "Region Setting Delete Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Region Setting Delete Test",
        500,
        22.0,
        45.0,
        false,
    )
    .await;

    let now = OffsetDateTime::now_utc();
    let tomorrow = now + time::Duration::days(1);

    let setting = create_test_region_setting(
        app.storage.clone(),
        region.id,
        200,
        800,
        18.0,
        26.0,
        now,
        tomorrow,
    )
    .await;

    let delete_setting_request = Request::builder()
        .uri(format!("/api/regions/settings/{}", setting.id))
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
        .uri(format!("/api/regions/settings/{}", setting.id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let verify_response = app.router.clone().oneshot(verify_request).await.unwrap();

    assert_eq!(verify_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_region_setting_validation() {
    let app = MockApp::new().await.with_region_handle();
    let group = create_test_group(app.storage.clone(), "Region Setting Validation Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;
    let region = create_test_region(
        app.storage.clone(),
        group.id,
        "Region Setting Validation Region",
        500,
        22.0,
        45.0,
        false,
    )
    .await;

    let now = OffsetDateTime::now_utc();
    let tomorrow = now + time::Duration::days(1);

    let invalid_light_setting = RegionSettingData {
        light_range: (1000, 500),
        temperature_range: (18.0, 26.0),
        humidity_range: (40.0, 60.0),
    };

    let invalid_light_request = Request::builder()
        .uri(format!("/api/regions/{}/settings", region.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateSettingRequest {
                target_id: region.id,
                data: invalid_light_setting,
                start_time: now,
                end_time: tomorrow,
            })
            .unwrap(),
        ))
        .unwrap();

    let invalid_light_response = app
        .router
        .clone()
        .oneshot(invalid_light_request)
        .await
        .unwrap();

    assert_eq!(invalid_light_response.status(), StatusCode::BAD_REQUEST);

    let invalid_temp_setting = RegionSettingData {
        light_range: (200, 800),
        temperature_range: (-60.0, 110.0),
        humidity_range: (40.0, 60.0),
    };

    let invalid_temp_request = Request::builder()
        .uri(format!("/api/regions/{}/settings", region.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateSettingRequest {
                target_id: region.id,
                data: invalid_temp_setting,
                start_time: now,
                end_time: tomorrow,
            })
            .unwrap(),
        ))
        .unwrap();

    let invalid_temp_response = app
        .router
        .clone()
        .oneshot(invalid_temp_request)
        .await
        .unwrap();

    assert_eq!(invalid_temp_response.status(), StatusCode::BAD_REQUEST);

    let valid_setting = RegionSettingData {
        light_range: (200, 800),
        temperature_range: (18.0, 26.0),
        humidity_range: (40.0, 60.0),
    };

    let invalid_time_request = Request::builder()
        .uri(format!("/api/regions/{}/settings", region.id))
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateSettingRequest {
                target_id: region.id,
                data: valid_setting,
                start_time: tomorrow,
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
}
