use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt;

use lumisync_api::models::CreateGroupRequest;
use lumisync_server::tests::{create_test_group, create_test_user_group};
use serde_json::json;

mod common;
use common::mock_app::MockApp;

#[tokio::test]
async fn test_create_group() {
    let app = MockApp::new().await.with_group_handle();

    let request = Request::builder()
        .uri("/api/groups")
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateGroupRequest {
                name: "Test Group".to_string(),
                description: Some("This is a test group".to_string()),
                users: vec![],
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let group_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(group_response["name"], json!("Test Group"));
    assert_eq!(group_response["description"], json!("This is a test group"));

    let request = Request::builder()
        .uri("/api/groups")
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateGroupRequest {
                name: "Test Group".to_string(),
                description: Some("This is another test group".to_string()),
                users: vec![app.admin.id],
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_get_user_groups() {
    let app = MockApp::new().await.with_group_handle();
    let group = create_test_group(app.storage.clone(), "User Group Test").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;

    let request = Request::builder()
        .uri("/api/groups")
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let groups: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert!(!groups.is_empty());
    assert!(groups.iter().any(|g| g["name"] == json!("User Group Test")));

    let request = Request::builder()
        .uri("/api/groups")
        .method(Method::GET)
        .header("Authorization", "Bearer invalid_token")
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_group_by_id() {
    let app = MockApp::new().await.with_group_handle();
    let group = create_test_group(app.storage.clone(), "ID Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;

    let request = Request::builder()
        .uri(format!("/api/groups/{}", group.id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let group_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(group_response["id"], json!(group.id));
    assert_eq!(group_response["name"], json!("ID Test Group"));

    let request = Request::builder()
        .uri("/api/groups/9999")
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_group() {
    let app = MockApp::new().await.with_group_handle();
    let group = create_test_group(app.storage.clone(), "Update Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;

    let request = Request::builder()
        .uri(format!("/api/groups/{}", group.id))
        .method(Method::PUT)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::from(
            serde_json::to_string(&CreateGroupRequest {
                name: "Updated Group".to_string(),
                description: Some("This is the updated description".to_string()),
                users: vec![app.admin.id],
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated_group: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(updated_group["id"], json!(group.id));
    assert_eq!(updated_group["name"], json!("Updated Group"));
    assert_eq!(
        updated_group["description"],
        json!("This is the updated description")
    );
}

#[tokio::test]
async fn test_delete_group() {
    let app = MockApp::new().await.with_group_handle();
    let group = create_test_group(app.storage.clone(), "Delete Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;

    let request = Request::builder()
        .uri(format!("/api/groups/{}", group.id))
        .method(Method::DELETE)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let request = Request::builder()
        .uri(format!("/api/groups/{}", group.id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_group_users() {
    let app = MockApp::new().await.with_group_handle();
    let group = create_test_group(app.storage.clone(), "User List Test Group").await;
    create_test_user_group(app.storage.clone(), app.admin.id, group.id, true).await;

    // Test successful retrieval of group users
    let request = Request::builder()
        .uri(format!("/api/groups/{}/users", group.id))
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let users: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert!(!users.is_empty());
    assert!(users.iter().any(|u| u["email"] == json!(app.admin.email)));

    // Test with non-existent group
    let request = Request::builder()
        .uri("/api/groups/9999/users")
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test with invalid token
    let request = Request::builder()
        .uri(format!("/api/groups/{}/users", group.id))
        .method(Method::GET)
        .header("Authorization", "Bearer invalid_token")
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
