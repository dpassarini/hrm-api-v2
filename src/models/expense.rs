use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::pagination::PaginationMeta;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Expense {
    pub id: Uuid,
    pub amount: BigDecimal,
    pub date: NaiveDate,
    pub description: String,
    pub project_id: Option<Uuid>,
    pub responsible_id: Uuid,
    pub tenant_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseProjectSummary {
    pub id: Uuid,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptInfo {
    pub attached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseWithProject {
    pub id: Uuid,
    pub amount: BigDecimal,
    pub date: NaiveDate,
    pub description: String,
    pub project_id: Option<Uuid>,
    pub responsible_id: Uuid,
    pub tenant_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub project: Option<ExpenseProjectSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<ReceiptInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ExpenseFilterQuery {
    pub description: Option<String>,
    pub project_id: Option<String>,
    pub responsible_id: Option<Uuid>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ExpensesResponse {
    pub expenses: Vec<ExpenseWithProject>,
    pub meta: PaginationMeta,
}

#[derive(Debug, Deserialize)]
pub struct CreateExpenseParams {
    pub expense: CreateExpensePayload,
    pub receipt: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateExpensePayload {
    pub amount: BigDecimal,
    pub date: NaiveDate,
    pub description: String,
    pub project_id: Option<Uuid>,
    pub responsible_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateExpenseParams {
    pub expense: UpdateExpensePayload,
    pub receipt: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateExpensePayload {
    pub amount: Option<BigDecimal>,
    pub date: Option<NaiveDate>,
    pub description: Option<String>,
    pub project_id: Option<Uuid>,
    pub responsible_id: Option<Uuid>,
}
