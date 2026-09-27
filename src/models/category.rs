use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Category {
    pub id: Uuid,
    pub name: Option<String>,
    pub tenant_id: Option<Uuid>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCategoryParams {
    pub category: CreateCategoryPayload,
}

#[derive(Debug, Deserialize)]
pub struct CreateCategoryPayload {
    pub name: String,
    pub global: Option<serde_json::Value>, // can be boolean or "true" string
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryParams {
    pub category: UpdateCategoryPayload,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryPayload {
    pub name: String,
}
