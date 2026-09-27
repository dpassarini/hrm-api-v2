use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    UnprocessableEntity(Vec<String>),
    UnprocessableEntityMsg(String),
    BadGateway(String),
    InternalServerError(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            AppError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            AppError::UnprocessableEntity(errors) => write!(f, "Unprocessable Entity: {:?}", errors),
            AppError::UnprocessableEntityMsg(msg) => write!(f, "Unprocessable Entity: {}", msg),
            AppError::BadGateway(msg) => write!(f, "Bad Gateway: {}", msg),
            AppError::InternalServerError(msg) => write!(f, "Internal Server Error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "Unauthorized",
                    "message": msg
                })),
            )
                .into_response(),
            AppError::Forbidden(msg) => (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "Forbidden",
                    "message": msg
                })),
            )
                .into_response(),
            AppError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Not Found",
                    "message": msg
                })),
            )
                .into_response(),
            AppError::UnprocessableEntity(errors) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "errors": errors
                })),
            )
                .into_response(),
            AppError::UnprocessableEntityMsg(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "error": msg
                })),
            )
                .into_response(),
            AppError::BadGateway(msg) => (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "Failed to connect to Identity Provider",
                    "message": msg
                })),
            )
                .into_response(),
            AppError::InternalServerError(msg) => {
                tracing::error!("Internal server error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Internal Server Error",
                        "message": msg
                    })),
                )
                    .into_response()
            }
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("Record not found".to_string()),
            _ => {
                tracing::error!("Database error: {:?}", err);
                AppError::InternalServerError(err.to_string())
            }
        }
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        AppError::Unauthorized(err.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::BadGateway(err.to_string())
    }
}
