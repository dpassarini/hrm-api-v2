use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::pagination::PaginationMeta;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Contact {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub tenant_id: Option<Uuid>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanySummary {
    pub id: Uuid,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactWithCompany {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub tenant_id: Option<Uuid>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub company: Option<CompanySummary>,
}

#[derive(Debug, Deserialize)]
pub struct ContactFilterQuery {
    pub name: Option<String>,
    pub email: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ContactsResponse {
    pub contacts: Vec<ContactWithCompany>,
    pub meta: PaginationMeta,
}

#[derive(Debug, Deserialize)]
pub struct CreateContactParams {
    pub contact: CreateContactPayload,
}

#[derive(Debug, Deserialize)]
pub struct CreateContactPayload {
    pub company_id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateContactParams {
    pub contact: UpdateContactPayload,
}

#[derive(Debug, Deserialize)]
pub struct UpdateContactPayload {
    pub company_id: Option<Uuid>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
}
