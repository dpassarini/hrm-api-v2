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
async fn test_projects_crud_flow() {
    let (app, _pool) = match common::setup_test_app().await {
        Some(res) => res,
        None => return,
    };

    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let token = common::create_valid_token(user_id, tenant_id, "user");

    // 1. Create Project with blank name -> 422
    let req = Request::builder()
        .method("POST")
        .uri("/projects")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "project": {
                "name": "   "
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 2. Create Valid Project -> 201
    let req = Request::builder()
        .method("POST")
        .uri("/projects")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "project": {
                "name": "HRM System Migration",
                "description": "Migrating to Rust Axum",
                "responsible_id": user_id,
                "status": "in_progress",
                "start_date": "2026-09-01",
                "end_date": "2026-12-31"
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let project: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let project_id = project["id"].as_str().unwrap();
    assert_eq!(project["name"], "HRM System Migration");
    assert_eq!(project["status"], "in_progress");

    // 3. Get Project by ID -> 200
    let req = Request::builder()
        .method("GET")
        .uri(format!("/projects/{}", project_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Update Project -> 200
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/projects/{}", project_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "project": {
                "status": "completed"
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let updated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(updated["status"], "completed");

    // 5. List Projects with status filter -> 200
    let req = Request::builder()
        .method("GET")
        .uri("/projects?status=completed&page=1&per_page=10")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let list_res: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(list_res["projects"].as_array().unwrap().len() >= 1);
}
