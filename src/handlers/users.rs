use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::time::Duration;

use crate::{auth::AuthUser, config::AppConfig, error::AppError};

pub async fn list_users(user: AuthUser) -> Result<Response, AppError> {
    let config = AppConfig::get();
    let url = format!("{}/users.json", config.unified_login_url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(AppError::from)?;

    let res = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.raw_token))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to Identity Provider: {}", e);
            AppError::BadGateway(e.to_string())
        })?;

    let status = res.status();
    if status.is_success() {
        let json_body: serde_json::Value = res.json().await.map_err(AppError::from)?;
        Ok((StatusCode::OK, Json(json_body)).into_response())
    } else {
        let err_text = res.text().await.unwrap_or_default();
        tracing::error!("Failed to fetch users from Identity Provider: Code {}, Body: {}", status, err_text);
        Ok((
            status,
            Json(serde_json::json!({
                "error": "Failed to fetch users from Identity Provider",
                "details": err_text
            })),
        )
            .into_response())
    }
}
