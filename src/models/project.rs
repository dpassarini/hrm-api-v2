use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{
    activity::ActivityWithContacts,
    expense::Expense,
    pagination::PaginationMeta,
};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub responsible_id: Uuid,
    pub status: String,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub tenant_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectWithDetails {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub responsible_id: Uuid,
    pub status: String,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub tenant_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub activities: Vec<ActivityWithContacts>,
    pub expenses: Vec<Expense>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectFilterQuery {
    pub name: Option<String>,
    pub status: Option<String>,
    pub responsible_id: Option<Uuid>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ProjectsResponse {
    pub projects: Vec<Project>,
    pub meta: PaginationMeta,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectParams {
    pub project: CreateProjectPayload,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectPayload {
    pub name: String,
    pub description: Option<String>,
    pub responsible_id: Uuid,
    pub status: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectParams {
    pub project: UpdateProjectPayload,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectPayload {
    pub name: Option<String>,
    pub description: Option<String>,
    pub responsible_id: Option<Uuid>,
    pub status: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}
