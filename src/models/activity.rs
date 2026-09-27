use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Activity {
    pub id: Uuid,
    pub project_id: Uuid,
    pub responsible_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub due_date: Option<DateTime<Utc>>,
    pub tenant_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityContactSummary {
    pub id: Uuid,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityWithContacts {
    pub id: Uuid,
    pub project_id: Uuid,
    pub responsible_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub due_date: Option<DateTime<Utc>>,
    pub tenant_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub contacts: Vec<ActivityContactSummary>,
}

#[derive(Debug, Deserialize)]
pub struct CreateActivityParams {
    pub activity: CreateActivityPayload,
}

#[derive(Debug, Deserialize)]
pub struct CreateActivityPayload {
    pub title: String,
    pub description: Option<String>,
    pub responsible_id: Uuid,
    pub status: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub contact_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateActivityParams {
    pub activity: UpdateActivityPayload,
}

#[derive(Debug, Deserialize)]
pub struct UpdateActivityPayload {
    pub title: Option<String>,
    pub description: Option<String>,
    pub responsible_id: Option<Uuid>,
    pub status: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub contact_ids: Option<Vec<Uuid>>,
}
