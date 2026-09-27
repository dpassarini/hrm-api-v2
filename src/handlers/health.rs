use axum::{response::IntoResponse, Json};
use serde_json::json;

pub async fn root() -> impl IntoResponse {
    Json(json!({ "status": "online" }))
}

pub async fn health() -> impl IntoResponse {
    Json(json!({ "status": "online" }))
}
