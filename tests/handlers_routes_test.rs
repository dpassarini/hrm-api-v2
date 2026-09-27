mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use hrm_api_v2::handlers::smart_input::{analyze, AnalyzeRequest};
use http_body_util::BodyExt;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn test_smart_input_analyze_validation_empty_input() {
    let _ = common::get_test_keys();
    let token = common::create_valid_token(Uuid::new_v4(), Uuid::new_v4(), "user");

    let app = axum::Router::new().route("/smart_input/analyze", axum::routing::post(analyze));

    let body = serde_json::to_string(&AnalyzeRequest {
        text: None,
        image: None,
        provider: None,
    })
    .unwrap();

    let request = Request::builder()
        .method("POST")
        .uri("/smart_input/analyze")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json["error"]
        .as_str()
        .unwrap()
        .contains("Informe um texto ou envie uma foto/imagem"));
}

#[tokio::test]
async fn test_unauthorized_request_without_token() {
    let app = axum::Router::new().route("/smart_input/analyze", axum::routing::post(analyze));

    let request = Request::builder()
        .method("POST")
        .uri("/smart_input/analyze")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
