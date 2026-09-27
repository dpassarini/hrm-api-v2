mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn test_activities_crud_flow() {
    let (app, pool) = match common::setup_test_app().await {
        Some(res) => res,
        None => return,
    };

    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let token = common::create_valid_token(user_id, tenant_id, "user");

    // Setup Project
    let proj_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query(
        r#"
        INSERT INTO projects (id, name, responsible_id, tenant_id, created_at, updated_at)
        VALUES ($1, 'Project For Activities', $2, $3, $4, $4)
        "#,
    )
    .bind(proj_id)
    .bind(user_id)
    .bind(tenant_id)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    // 1. Create Activity for project with blank title -> 422
    let req = Request::builder()
        .method("POST")
        .uri(format!("/projects/{}/activities", proj_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "activity": {
                "title": "   "
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 2. Create Valid Activity -> 201
    let req = Request::builder()
        .method("POST")
        .uri(format!("/projects/{}/activities", proj_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "activity": {
                "title": "Setup CI/CD Pipeline",
                "responsible_id": user_id,
                "status": "pending",
                "due_date": "2026-10-15T18:00:00Z"
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let act: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let act_id = act["id"].as_str().unwrap();
    assert_eq!(act["title"], "Setup CI/CD Pipeline");
    assert_eq!(act["status"], "pending");

    // 3. Update Activity -> 200
    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/activities/{}", act_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "activity": {
                "status": "completed"
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let updated_act: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(updated_act["status"], "completed");
}
