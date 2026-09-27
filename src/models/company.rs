use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::pagination::PaginationMeta;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Company {
    pub id: Uuid,
    pub name: Option<String>,
    pub cnpj: Option<String>,
    pub website: Option<String>,
    pub categories: Option<Vec<String>>,
    pub tenant_id: Option<Uuid>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CompanyFilterQuery {
    pub name: Option<String>,
    pub category: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CompaniesResponse {
    pub companies: Vec<Company>,
    pub meta: PaginationMeta,
}

#[derive(Debug, Deserialize)]
pub struct CreateCompanyParams {
    pub company: CreateCompanyPayload,
}

#[derive(Debug, Deserialize)]
pub struct CreateCompanyPayload {
    pub name: String,
    pub cnpj: Option<String>,
    pub website: Option<String>,
    pub categories: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCompanyParams {
    pub company: UpdateCompanyPayload,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCompanyPayload {
    pub name: Option<String>,
    pub cnpj: Option<String>,
    pub website: Option<String>,
    pub categories: Option<Vec<String>>,
}
