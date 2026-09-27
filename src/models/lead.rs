use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{
    company::Company,
    contact::Contact,
    pagination::PaginationMeta,
};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Lead {
    pub id: Uuid,
    pub name: String,
    pub company_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub website: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub source: Option<String>,
    pub status: String,
    pub notes: Option<String>,
    pub categories: Option<Vec<String>>,
    pub responsible_id: Option<Uuid>,
    pub tenant_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct LeadFilterQuery {
    pub name: Option<String>,
    pub company_name: Option<String>,
    pub status: Option<String>,
    pub category: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct LeadsResponse {
    pub leads: Vec<Lead>,
    pub meta: PaginationMeta,
}

#[derive(Debug, Deserialize)]
pub struct CreateLeadParams {
    pub lead: CreateLeadPayload,
}

#[derive(Debug, Deserialize)]
pub struct CreateLeadPayload {
    pub name: String,
    pub company_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub website: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub source: Option<String>,
    pub status: Option<String>,
    pub notes: Option<String>,
    pub categories: Option<Vec<String>>,
    pub responsible_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLeadParams {
    pub lead: UpdateLeadPayload,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLeadPayload {
    pub name: Option<String>,
    pub company_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub website: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub source: Option<String>,
    pub status: Option<String>,
    pub notes: Option<String>,
    pub categories: Option<Vec<String>>,
    pub responsible_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ConvertLeadResponse {
    pub message: String,
    pub company: Company,
    pub contact: Contact,
    pub lead: Lead,
}
