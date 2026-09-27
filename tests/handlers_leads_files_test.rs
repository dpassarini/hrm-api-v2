mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn test_leads_files_endpoints() {
    let (app, pool) = match common::setup_test_app().await {
        Some(res) => res,
        None => return,
    };

    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let token = common::create_valid_token(user_id, tenant_id, "user");

    let file_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query(
        r#"
        INSERT INTO leads_files (id, tenant_id, created_at, updated_at)
        VALUES ($1, $2, $3, $3)
        "#,
    )
    .bind(file_id)
    .bind(tenant_id)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    let _ = hrm_api_v2::services::storage::StorageService::save_attachment(
        &pool,
        "LeadsFile",
        file_id,
        "file",
        "test_leads.csv",
        "text/csv",
        b"name,email\nLead A,a@test.com",
    )
    .await;

    // 1. List leads files -> 200
    let req = Request::builder()
        .method("GET")
        .uri("/leads_files?page=1&per_page=10")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(list["leads_files"].as_array().unwrap().len() >= 1);

    // 2. Get leads file by ID -> 200
    let req = Request::builder()
        .method("GET")
        .uri(format!("/leads_files/{}", file_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let detail: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(detail["id"], file_id.to_string());

    // 3. Get nonexistent leads file -> 404
    let req = Request::builder()
        .method("GET")
        .uri(format!("/leads_files/{}", Uuid::new_v4()))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
