use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use crate::{
    auth::AuthUser,
    error::AppError,
    services::smart_input::{builder::EntityBuilderService, extractor::ExtractorService},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyzeRequest {
    pub text: Option<String>,
    pub image: Option<Value>,
    pub provider: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitRequest {
    pub payload: Option<Value>,
}

pub async fn analyze(
    _user: AuthUser,
    Json(body): Json<AnalyzeRequest>,
) -> Result<impl IntoResponse, AppError> {
    let text = body.text.as_deref().filter(|s| !s.trim().is_empty());

    // Normalize image
    let mut image_tuple: Option<(&str, &str)> = None;
    let mut _owned_mime = String::new();
    let mut _owned_data = String::new();

    if let Some(img_val) = &body.image {
        match img_val {
            Value::String(s) => {
                if s.starts_with("data:") {
                    if let Some(idx) = s.find(";base64,") {
                        _owned_mime = s[5..idx].to_string();
                        _owned_data = s[idx + 8..].to_string();
                        image_tuple = Some((&_owned_mime, &_owned_data));
                    } else {
                        _owned_mime = "image/jpeg".to_string();
                        _owned_data = s.clone();
                        image_tuple = Some((&_owned_mime, &_owned_data));
                    }
                } else {
                    _owned_mime = "image/jpeg".to_string();
                    _owned_data = s.clone();
                    image_tuple = Some((&_owned_mime, &_owned_data));
                }
            }
            Value::Object(map) => {
                let ct = map
                    .get("mime_type")
                    .or_else(|| map.get("content_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("image/jpeg");
                let data = map.get("data").and_then(|v| v.as_str()).unwrap_or("");
                if !data.is_empty() {
                    _owned_mime = ct.to_string();
                    _owned_data = if data.starts_with("data:") {
                        if let Some(idx) = data.find(";base64,") {
                            data[idx + 8..].to_string()
                        } else {
                            data.to_string()
                        }
                    } else {
                        data.to_string()
                    };
                    image_tuple = Some((&_owned_mime, &_owned_data));
                }
            }
            _ => {}
        }
    }

    if text.is_none() && image_tuple.is_none() {
        return Err(AppError::UnprocessableEntityMsg(
            "Informe um texto ou envie uma foto/imagem para análise.".to_string(),
        ));
    }

    let result = ExtractorService::extract(
        text,
        image_tuple,
        body.provider.as_deref(),
    )
    .await;

    match result {
        Some(json_data) => Ok(Json(json_data)),
        None => Err(AppError::InternalServerError(
            "Não foi possível extrair informações dos dados fornecidos.".to_string(),
        )),
    }
}

pub async fn commit(
    State(pool): State<PgPool>,
    user: AuthUser,
    Json(body): Json<CommitRequest>,
) -> Result<impl IntoResponse, AppError> {
    let payload = body
        .payload
        .ok_or_else(|| AppError::UnprocessableEntityMsg("Payload cannot be empty".to_string()))?;

    let message = EntityBuilderService::commit(&pool, &payload, &user).await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "message": message })),
    ))
}
