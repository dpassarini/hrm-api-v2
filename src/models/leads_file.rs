use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::pagination::PaginationMeta;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LeadsFile {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LeadsFileLine {
    pub id: Uuid,
    pub leads_file_id: Uuid,
    pub status: i32, // 0: salvo, 1: processando, 2: erro, 3: sucesso
    pub data: serde_json::Value,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadsFileSummary {
    pub id: Uuid,
    pub filename: String,
    pub byte_size: i64,
    pub created_at: DateTime<Utc>,
    pub total_lines: i64,
    pub success_count: i64,
    pub error_count: i64,
    pub processing_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadsFileErrorLine {
    pub id: Uuid,
    pub data: serde_json::Value,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadsFileDetail {
    pub id: Uuid,
    pub filename: String,
    pub created_at: DateTime<Utc>,
    pub total_lines: i64,
    pub success_count: i64,
    pub error_count: i64,
    pub processing_count: i64,
    pub error_lines: Vec<LeadsFileErrorLine>,
}

#[derive(Debug, Serialize)]
pub struct LeadsFilesResponse {
    pub leads_files: Vec<LeadsFileSummary>,
    pub meta: PaginationMeta,
}

#[derive(Debug, Serialize)]
pub struct CreateLeadsFileResponse {
    pub message: String,
    pub id: Uuid,
}
