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
async fn test_companies_crud_flow() {
    let (app, _pool) = match common::setup_test_app().await {
        Some(res) => res,
        None => return, // Skip if database is not available
    };

    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let token = common::create_valid_token(user_id, tenant_id, "admin");

    // 1. Create Category first (so pre-registration check passes)
    let cat_body = json!({
        "category": {
            "name": "Tecnologia"
        }
    });
    let req = Request::builder()
        .method("POST")
        .uri("/categories")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(cat_body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 2. Create Company with invalid category -> Should fail 422
    let invalid_comp_body = json!({
        "company": {
            "name": "Invalid Tech",
            "categories": ["Inexistente"]
        }
    });
    let req = Request::builder()
        .method("POST")
        .uri("/companies")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(invalid_comp_body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 3. Create Company with blank name -> Should fail 422
    let blank_comp_body = json!({
        "company": {
            "name": "  ",
            "categories": ["Tecnologia"]
        }
    });
    let req = Request::builder()
        .method("POST")
        .uri("/companies")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(blank_comp_body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 4. Create Valid Company -> 201 Created
    let valid_comp_body = json!({
        "company": {
            "name": "Acme Rust Corp",
            "cnpj": "12.345.678/0001-90",
            "website": "https://acme.rust",
            "categories": ["Tecnologia"]
        }
    });
    let req = Request::builder()
        .method("POST")
        .uri("/companies")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(valid_comp_body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let created_company: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let comp_id = created_company["id"].as_str().unwrap();
    assert_eq!(created_company["name"], "Acme Rust Corp");

    // 5. Get Company by ID -> 200 OK
    let req = Request::builder()
        .method("GET")
        .uri(format!("/companies/{}", comp_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 6. Get Nonexistent Company -> 404
    let req = Request::builder()
        .method("GET")
        .uri(format!("/companies/{}", Uuid::new_v4()))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 7. Update Company -> 200 OK
    let update_body = json!({
        "company": {
            "name": "Acme Rust Corporation",
            "website": "https://corp.acme.rust"
        }
    });
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/companies/{}", comp_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(update_body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let updated_comp: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(updated_comp["name"], "Acme Rust Corporation");
    assert_eq!(updated_comp["website"], "https://corp.acme.rust");

    // 8. List Companies with Name filter
    let req = Request::builder()
        .method("GET")
        .uri("/companies?name=Acme&page=1&per_page=10")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let list_res: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(list_res["companies"].as_array().unwrap().len() >= 1);
    assert_eq!(list_res["meta"]["total_pages"], 1);
}
