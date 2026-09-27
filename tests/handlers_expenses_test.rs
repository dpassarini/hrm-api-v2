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
async fn test_expenses_crud_flow() {
    let (app, _pool) = match common::setup_test_app().await {
        Some(res) => res,
        None => return,
    };

    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let token = common::create_valid_token(user_id, tenant_id, "user");

    // 1. Create Expense with blank description -> 422
    let req = Request::builder()
        .method("POST")
        .uri("/expenses")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "expense": {
                "amount": "150.50",
                "date": "2026-09-27",
                "description": "   "
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 2. Create Valid Expense -> 201
    let req = Request::builder()
        .method("POST")
        .uri("/expenses")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "expense": {
                "amount": "250.75",
                "date": "2026-09-27",
                "description": "Almoço Comercial (Restaurante Central)",
                "establishment": "Restaurante Central"
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let expense: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let expense_id = expense["id"].as_str().unwrap();
    assert!(expense["amount"].as_str().unwrap().starts_with("250.75"));
    assert_eq!(expense["description"], "Almoço Comercial (Restaurante Central)");

    // 3. Get Expense by ID -> 200
    let req = Request::builder()
        .method("GET")
        .uri(format!("/expenses/{}", expense_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Update Expense -> 200
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/expenses/{}", expense_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "expense": {
                "amount": "270.00"
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let updated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(updated["amount"].as_str().unwrap().starts_with("270"));

    // 5. List Expenses with filter -> 200
    let req = Request::builder()
        .method("GET")
        .uri("/expenses?page=1&per_page=10")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let list_res: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(list_res["expenses"].as_array().unwrap().len() >= 1);
}
