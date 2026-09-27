use axum::{
    http::StatusCode,
    response::IntoResponse,
};
use hrm_api_v2::error::AppError;
use http_body_util::BodyExt;

#[tokio::test]
async fn test_error_status_codes_and_payloads() {
    // 1. Unauthorized
    let err = AppError::Unauthorized("Invalid token".to_string());
    let res = err.into_response();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "Unauthorized");
    assert_eq!(json["message"], "Invalid token");

    // 2. Forbidden
    let err = AppError::Forbidden("No access".to_string());
    let res = err.into_response();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "Forbidden");
    assert_eq!(json["message"], "No access");

    // 3. NotFound
    let err = AppError::NotFound("Lead not found".to_string());
    let res = err.into_response();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "Not Found");
    assert_eq!(json["message"], "Lead not found");

    // 4. UnprocessableEntity (list)
    let err = AppError::UnprocessableEntity(vec!["Name can't be blank".to_string()]);
    let res = err.into_response();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["errors"][0], "Name can't be blank");

    // 5. UnprocessableEntityMsg
    let err = AppError::UnprocessableEntityMsg("Lead já convertido".to_string());
    let res = err.into_response();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "Lead já convertido");

    // 6. BadGateway
    let err = AppError::BadGateway("Connection timeout".to_string());
    let res = err.into_response();
    assert_eq!(res.status(), StatusCode::BAD_GATEWAY);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "Failed to connect to Identity Provider");

    // 7. InternalServerError
    let err = AppError::InternalServerError("Database crashed".to_string());
    let res = err.into_response();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "Internal Server Error");
}

#[test]
fn test_error_display() {
    assert_eq!(
        format!("{}", AppError::Unauthorized("foo".to_string())),
        "Unauthorized: foo"
    );
    assert_eq!(
        format!("{}", AppError::Forbidden("bar".to_string())),
        "Forbidden: bar"
    );
    assert_eq!(
        format!("{}", AppError::NotFound("baz".to_string())),
        "Not Found: baz"
    );
}
