mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use hrm_api_v2::handlers::health;
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn test_root_endpoint() {
    let app = axum::Router::new().route("/", axum::routing::get(health::root));

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["status"], "online");
}

#[tokio::test]
async fn test_health_up_endpoint() {
    let app = axum::Router::new().route("/up", axum::routing::get(health::health));

    let response = app
        .oneshot(Request::builder().uri("/up").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["status"], "online");
}
