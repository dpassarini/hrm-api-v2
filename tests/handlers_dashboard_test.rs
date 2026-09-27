mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn test_dashboard_stats_endpoint() {
    let (app, _pool) = match common::setup_test_app().await {
        Some(res) => res,
        None => return,
    };

    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let token = common::create_valid_token(user_id, tenant_id, "user");

    let req = Request::builder()
        .method("GET")
        .uri("/dashboard/stats")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let stats: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(stats["companies_count"].is_number());
    assert!(stats["contacts_count"].is_number());
    assert!(stats["leads_count"]["total"].is_number());
    assert!(stats["projects_count"]["total"].is_number());
    assert!(stats["activities_count"]["total"].is_number());
    assert!(stats["total_expense_amount"].is_number());
}
