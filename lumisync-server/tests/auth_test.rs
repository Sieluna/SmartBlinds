use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt;

use lumisync_api::models::{LoginRequest, RegisterRequest};
use serde_json::json;

mod common;
use common::mock_app::MockApp;

#[tokio::test]
async fn test_register() {
    let app = MockApp::new().await.with_auth_handle();

    let request = Request::builder()
        .uri("/api/auth/register")
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&RegisterRequest {
                email: "new_user@test.com".to_string(),
                password: "password123".to_string(),
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let request = Request::builder()
        .uri("/api/auth/register")
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&RegisterRequest {
                email: "new_user@test.com".to_string(),
                password: "password123".to_string(),
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_login() {
    let app = MockApp::new().await.with_auth_handle();

    let request = Request::builder()
        .uri("/api/auth/register")
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&RegisterRequest {
                email: "login_test@test.com".to_string(),
                password: "password123".to_string(),
            })
            .unwrap(),
        ))
        .unwrap();

    let _ = app.router.clone().oneshot(request).await.unwrap();

    let request = Request::builder()
        .uri("/api/auth/login")
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&LoginRequest {
                email: "login_test@test.com".to_string(),
                password: "password123".to_string(),
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let request = Request::builder()
        .uri("/api/auth/login")
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&LoginRequest {
                email: "login_test@test.com".to_string(),
                password: "wrong_password".to_string(),
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let request = Request::builder()
        .uri("/api/auth/login")
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&LoginRequest {
                email: "non_existent@test.com".to_string(),
                password: "password123".to_string(),
            })
            .unwrap(),
        ))
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_refresh_token() {
    let app = MockApp::new().await.with_auth_handle();

    let request = Request::builder()
        .uri("/api/auth/refresh")
        .method(Method::POST)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let request = Request::builder()
        .uri("/api/auth/refresh")
        .method(Method::POST)
        .header("Authorization", "Bearer invalid_token")
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_current_user() {
    let app = MockApp::new().await.with_auth_handle();

    let request = Request::builder()
        .uri("/api/auth/me")
        .method(Method::GET)
        .header("Authorization", format!("Bearer {}", app.token))
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let user_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(user_response["email"], json!("admin@test.com"));
    assert_eq!(user_response["role"], json!("admin"));

    let request = Request::builder()
        .uri("/api/auth/me")
        .method(Method::GET)
        .header("Authorization", "Bearer invalid_token")
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
