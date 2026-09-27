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
async fn test_contacts_crud_and_validation() {
    let (app, pool) = match common::setup_test_app().await {
        Some(res) => res,
        None => return,
    };

    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let token = common::create_valid_token(user_id, tenant_id, "user");

    // Setup Category & Company
    let comp_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query(
        r#"
        INSERT INTO companies (id, name, tenant_id, created_at, updated_at)
        VALUES ($1, 'Test Company Contacts', $2, $3, $3)
        "#,
    )
    .bind(comp_id)
    .bind(tenant_id)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    // 1. Create Contact with missing company_id -> 422
    let req = Request::builder()
        .method("POST")
        .uri("/contacts")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "contact": {
                "name": "John Doe",
                "email": "john@test.com"
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 2. Create Contact with blank name -> 422
    let req = Request::builder()
        .method("POST")
        .uri("/contacts")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "contact": {
                "company_id": comp_id,
                "name": "   "
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 3. Create Valid Contact -> 201
    let req = Request::builder()
        .method("POST")
        .uri("/contacts")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "contact": {
                "company_id": comp_id,
                "name": "John Doe",
                "email": "john.doe@example.com",
                "phone": "+55 11 99999-8888",
                "job_title": "CTO"
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let contact: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let contact_id = contact["id"].as_str().unwrap();
    assert_eq!(contact["name"], "John Doe");
    assert_eq!(contact["job_title"], "CTO");

    // 4. Get Contact by ID -> 200
    let req = Request::builder()
        .method("GET")
        .uri(format!("/contacts/{}", contact_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 5. Update Contact -> 200
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/contacts/{}", contact_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "contact": {
                "job_title": "Chief Technology Officer"
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 6. List Contacts with company_id filter -> 200
    let req = Request::builder()
        .method("GET")
        .uri(format!("/contacts?company_id={}", comp_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
