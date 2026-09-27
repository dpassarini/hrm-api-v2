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
async fn test_leads_crud_and_conversion_flow() {
    let (app, _pool) = match common::setup_test_app().await {
        Some(res) => res,
        None => return,
    };

    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let token = common::create_valid_token(user_id, tenant_id, "user");

    // 1. Create Lead with blank name -> 422
    let req = Request::builder()
        .method("POST")
        .uri("/leads")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "lead": {
                "name": "   "
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 2. Create Valid Lead -> 201
    let req = Request::builder()
        .method("POST")
        .uri("/leads")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "lead": {
                "name": "Carlos Silva",
                "company_name": "Silva Solucoes",
                "email": "carlos@silva.com",
                "phone": "+55 11 98888-7777",
                "job_title": "Diretor",
                "website": "https://silva.com",
                "categories": ["Consultoria"]
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let lead: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let lead_id = lead["id"].as_str().unwrap();
    assert_eq!(lead["name"], "Carlos Silva");
    assert_eq!(lead["status"], "new");

    // 3. Get Lead by ID -> 200
    let req = Request::builder()
        .method("GET")
        .uri(format!("/leads/{}", lead_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Update Lead -> 200
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/leads/{}", lead_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({
            "lead": {
                "notes": "Interesse em proposta comercial"
            }
        }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 5. Convert Lead -> 200
    let req = Request::builder()
        .method("POST")
        .uri(format!("/leads/{}/convert", lead_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let conversion_res: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(conversion_res["company"]["name"], "Silva Solucoes");
    assert_eq!(conversion_res["contact"]["name"], "Carlos Silva");
    assert_eq!(conversion_res["lead"]["status"], "converted");

    // 6. Convert again -> 422 (already converted)
    let req = Request::builder()
        .method("POST")
        .uri(format!("/leads/{}/convert", lead_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 7. Delete Lead -> 204
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/leads/{}", lead_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}
