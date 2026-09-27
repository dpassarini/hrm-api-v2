use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::OnceLock;
use uuid::Uuid;

use crate::{config::AppConfig, error::AppError};

static DECODING_KEY: OnceLock<DecodingKey> = OnceLock::new();

fn get_decoding_key() -> Result<&'static DecodingKey, AppError> {
    if let Some(key) = DECODING_KEY.get() {
        return Ok(key);
    }

    let config = AppConfig::get();
    let key_pem = fs::read_to_string(&config.public_key_path).map_err(|e| {
        tracing::error!("Failed to read public key at '{}': {}", config.public_key_path, e);
        AppError::InternalServerError(format!("Failed to read public key: {}", e))
    })?;

    let decoding_key = DecodingKey::from_rsa_pem(key_pem.as_bytes()).map_err(|e| {
        tracing::error!("Failed to parse RSA public key: {}", e);
        AppError::InternalServerError(format!("Failed to parse RSA public key: {}", e))
    })?;

    let _ = DECODING_KEY.set(decoding_key);
    Ok(DECODING_KEY.get().unwrap())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub tenant_id: Option<String>,
    pub email: Option<String>,
    pub name: Option<String>,
    pub profile: Option<String>,
    pub iss: Option<String>,
    pub aud: Option<serde_json::Value>,
    pub exp: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: String,
    pub tenant_id: Uuid,
    pub email: Option<String>,
    pub name: Option<String>,
    pub profile: Option<String>,
    pub raw_token: String,
}

impl AuthUser {
    pub fn is_super_admin(&self) -> bool {
        matches!(
            self.profile.as_deref(),
            Some("super_admin") | Some("super_user")
        )
    }

    pub fn is_admin(&self) -> bool {
        self.is_super_admin() || matches!(self.profile.as_deref(), Some("admin"))
    }

    pub fn is_app_client(&self) -> bool {
        self.user_id.starts_with("app_")
    }

    pub fn user_uuid(&self) -> Option<Uuid> {
        Uuid::parse_str(&self.user_id).ok()
    }
}

pub fn verify_jwt(token: &str) -> Result<AuthUser, AppError> {
    let key = get_decoding_key()?;

    let header = decode_header(token).map_err(|e| AppError::Unauthorized(e.to_string()))?;
    if header.alg != Algorithm::RS256 {
        return Err(AppError::Unauthorized("Invalid algorithm, expected RS256".to_string()));
    }

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["unified_login"]);
    validation.set_required_spec_claims(&["exp"]);
    validation.validate_aud = false; // We validate aud manually below to support single string or array

    let token_data = decode::<Claims>(token, key, &validation)
        .map_err(|e| AppError::Unauthorized(e.to_string()))?;

    let claims = token_data.claims;

    // Validate audience: must contain "hrm-api" or "crm-api"
    let valid_aud = match &claims.aud {
        Some(serde_json::Value::String(s)) => s == "hrm-api" || s == "crm-api",
        Some(serde_json::Value::Array(arr)) => arr.iter().any(|v| {
            if let serde_json::Value::String(s) = v {
                s == "hrm-api" || s == "crm-api"
            } else {
                false
            }
        }),
        _ => false,
    };

    if !valid_aud {
        return Err(AppError::Unauthorized("Invalid audience".to_string()));
    }

    // Validate tenant_id
    let tenant_id_str = claims.tenant_id.as_deref().unwrap_or("");
    if tenant_id_str.is_empty() || tenant_id_str == "missing" {
        return Err(AppError::Unauthorized("Missing tenant context in token".to_string()));
    }

    let tenant_uuid = Uuid::parse_str(tenant_id_str)
        .map_err(|_| AppError::Unauthorized("Invalid tenant UUID in token".to_string()))?;

    Ok(AuthUser {
        user_id: claims.sub,
        tenant_id: tenant_uuid,
        email: claims.email,
        name: claims.name,
        profile: claims.profile,
        raw_token: token.to_string(),
    })
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .ok_or_else(|| AppError::Unauthorized("Missing Authorization Header".to_string()))?
            .to_str()
            .map_err(|_| AppError::Unauthorized("Invalid Authorization Header".to_string()))?;

        if !auth_header.starts_with("Bearer ") && !auth_header.starts_with("bearer ") {
            let token = auth_header.split_whitespace().last().unwrap_or("");
            if token.is_empty() {
                return Err(AppError::Unauthorized("Invalid Authorization Token Format".to_string()));
            }
            return verify_jwt(token);
        }

        let token = &auth_header[7..].trim();
        if token.is_empty() {
            return Err(AppError::Unauthorized("Invalid Authorization Token Format".to_string()));
        }

        verify_jwt(token)
    }
}
