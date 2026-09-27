use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::AppError,
    models::{
        expense::{
            CreateExpenseParams, ExpenseFilterQuery, ExpenseProjectSummary, ExpenseWithProject,
            ExpensesResponse, ReceiptInfo, UpdateExpenseParams,
        },
        pagination::PaginationMeta,
    },
    services::storage::StorageService,
};

#[derive(Debug, serde::Deserialize)]
pub struct ReceiptQuery {
    pub disposition: Option<String>,
}

pub async fn list_expenses(
    State(pool): State<PgPool>,
    user: AuthUser,
    Query(query): Query<ExpenseFilterQuery>,
) -> Result<impl IntoResponse, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).max(1).min(100);
    let offset = (page - 1) * per_page;

    let desc_filter = query.description.as_deref().map(|d| format!("%{}%", d.trim()));

    let (filter_proj_none, filter_proj_id) = match query.project_id.as_deref() {
        Some("none") => (true, None),
        Some(s) => (false, Uuid::parse_str(s).ok()),
        None => (false, None),
    };

    let total_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM expenses e
        WHERE e.tenant_id = $1
          AND ($2::text IS NULL OR e.description ILIKE $2)
          AND ($3::uuid IS NULL OR e.responsible_id = $3)
          AND ($4::date IS NULL OR e.date >= $4)
          AND ($5::date IS NULL OR e.date <= $5)
          AND (
            CASE
              WHEN $6 = true THEN e.project_id IS NULL
              WHEN $7::uuid IS NOT NULL THEN e.project_id = $7
              ELSE true
            END
          )
        "#,
    )
    .bind(user.tenant_id)
    .bind(desc_filter.as_deref())
    .bind(query.responsible_id)
    .bind(query.start_date)
    .bind(query.end_date)
    .bind(filter_proj_none)
    .bind(filter_proj_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    let rows = sqlx::query(
        r#"
        SELECT e.id, e.amount, e.date, e.description, e.project_id, e.responsible_id, e.tenant_id,
               e.created_at, e.updated_at, p.id as proj_id, p.name as proj_name, p.status as proj_status
        FROM expenses e
        LEFT JOIN projects p ON p.id = e.project_id
        WHERE e.tenant_id = $1
          AND ($2::text IS NULL OR e.description ILIKE $2)
          AND ($3::uuid IS NULL OR e.responsible_id = $3)
          AND ($4::date IS NULL OR e.date >= $4)
          AND ($5::date IS NULL OR e.date <= $5)
          AND (
            CASE
              WHEN $6 = true THEN e.project_id IS NULL
              WHEN $7::uuid IS NOT NULL THEN e.project_id = $7
              ELSE true
            END
          )
        ORDER BY e.date DESC, e.created_at DESC
        LIMIT $8 OFFSET $9
        "#,
    )
    .bind(user.tenant_id)
    .bind(desc_filter.as_deref())
    .bind(query.responsible_id)
    .bind(query.start_date)
    .bind(query.end_date)
    .bind(filter_proj_none)
    .bind(filter_proj_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let expenses = rows
        .into_iter()
        .map(|r| {
            let proj_id: Option<Uuid> = r.get("proj_id");
            let proj_name: Option<String> = r.get("proj_name");
            let proj_status: Option<String> = r.get("proj_status");
            let amount: BigDecimal = r.get("amount");
            let date: NaiveDate = r.get("date");
            let created_at: DateTime<Utc> = r.get("created_at");
            let updated_at: DateTime<Utc> = r.get("updated_at");

            ExpenseWithProject {
                id: r.get("id"),
                amount,
                date,
                description: r.get("description"),
                project_id: r.get("project_id"),
                responsible_id: r.get("responsible_id"),
                tenant_id: r.get("tenant_id"),
                created_at,
                updated_at,
                project: proj_id.map(|pid| ExpenseProjectSummary {
                    id: pid,
                    name: proj_name.unwrap_or_default(),
                    status: proj_status.unwrap_or_default(),
                }),
                receipt: None,
            }
        })
        .collect();

    let meta = PaginationMeta::new(total_count, per_page);
    Ok(Json(ExpensesResponse { expenses, meta }))
}

pub async fn get_expense(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let r = sqlx::query(
        r#"
        SELECT e.id, e.amount, e.date, e.description, e.project_id, e.responsible_id, e.tenant_id,
               e.created_at, e.updated_at, p.id as proj_id, p.name as proj_name, p.status as proj_status
        FROM expenses e
        LEFT JOIN projects p ON p.id = e.project_id
        WHERE e.id = $1 AND e.tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Expense not found".to_string()))?;

    let receipt_row = sqlx::query(
        r#"
        SELECT b.filename, b.byte_size, b.content_type, b.created_at
        FROM active_storage_attachments a
        JOIN active_storage_blobs b ON b.id = a.blob_id
        WHERE a.record_type = 'Expense' AND a.record_id = $1 AND a.name = 'receipt'
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?;

    let receipt_info = match receipt_row {
        Some(b) => ReceiptInfo {
            attached: true,
            filename: Some(b.get("filename")),
            byte_size: Some(b.get("byte_size")),
            content_type: b.get("content_type"),
            created_at: Some(b.get("created_at")),
        },
        None => ReceiptInfo {
            attached: false,
            filename: None,
            byte_size: None,
            content_type: None,
            created_at: None,
        },
    };

    let proj_id: Option<Uuid> = r.get("proj_id");
    let proj_name: Option<String> = r.get("proj_name");
    let proj_status: Option<String> = r.get("proj_status");
    let amount: BigDecimal = r.get("amount");
    let date: NaiveDate = r.get("date");
    let created_at: DateTime<Utc> = r.get("created_at");
    let updated_at: DateTime<Utc> = r.get("updated_at");

    let expense = ExpenseWithProject {
        id: r.get("id"),
        amount,
        date,
        description: r.get("description"),
        project_id: r.get("project_id"),
        responsible_id: r.get("responsible_id"),
        tenant_id: r.get("tenant_id"),
        created_at,
        updated_at,
        project: proj_id.map(|pid| ExpenseProjectSummary {
            id: pid,
            name: proj_name.unwrap_or_default(),
            status: proj_status.unwrap_or_default(),
        }),
        receipt: Some(receipt_info),
    };

    Ok(Json(expense))
}

pub async fn create_expense(
    State(pool): State<PgPool>,
    user: AuthUser,
    Json(payload): Json<CreateExpenseParams>,
) -> Result<impl IntoResponse, AppError> {
    let p = payload.expense;
    let mut errors = Vec::new();

    let description = p.description.trim().to_string();
    if description.is_empty() {
        errors.push("Description can't be blank".to_string());
    }

    if p.amount <= BigDecimal::from(0) {
        errors.push("Amount must be greater than 0".to_string());
    }

    let responsible_id = p
        .responsible_id
        .or_else(|| user.user_uuid())
        .unwrap_or(user.tenant_id);

    let mut project_summary: Option<ExpenseProjectSummary> = None;
    if let Some(pid) = p.project_id {
        let proj = sqlx::query(
            r#"SELECT id, name, status FROM projects WHERE id = $1 AND tenant_id = $2"#,
        )
        .bind(pid)
        .bind(user.tenant_id)
        .fetch_optional(&pool)
        .await
        .map_err(AppError::from)?;

        match proj {
            Some(proj) => {
                let status: String = proj.get("status");
                if status == "completed" || status == "cancelled" {
                    errors.push("Project cannot add expenses to a completed or cancelled project".to_string());
                } else {
                    project_summary = Some(ExpenseProjectSummary {
                        id: proj.get("id"),
                        name: proj.get("name"),
                        status,
                    });
                }
            }
            None => {
                errors.push("Project not found".to_string());
            }
        }
    }

    if !errors.is_empty() {
        return Err(AppError::UnprocessableEntity(errors));
    }

    let id = Uuid::new_v4();
    let now = Utc::now();

    let row = sqlx::query(
        r#"
        INSERT INTO expenses (id, amount, date, description, project_id, responsible_id, tenant_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
        RETURNING id, amount, date, description, project_id, responsible_id, tenant_id, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(p.amount)
    .bind(p.date)
    .bind(description)
    .bind(p.project_id)
    .bind(responsible_id)
    .bind(user.tenant_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    if let Some(receipt_val) = payload.receipt {
        handle_receipt_attachment(&pool, id, &receipt_val).await;
    }

    let amount: BigDecimal = row.get("amount");
    let date: NaiveDate = row.get("date");
    let created_at: DateTime<Utc> = row.get("created_at");
    let updated_at: DateTime<Utc> = row.get("updated_at");

    let resp = ExpenseWithProject {
        id: row.get("id"),
        amount,
        date,
        description: row.get("description"),
        project_id: row.get("project_id"),
        responsible_id: row.get("responsible_id"),
        tenant_id: row.get("tenant_id"),
        created_at,
        updated_at,
        project: project_summary,
        receipt: None,
    };

    Ok((StatusCode::CREATED, Json(resp)))
}

pub async fn update_expense(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateExpenseParams>,
) -> Result<impl IntoResponse, AppError> {
    let existing = sqlx::query(
        r#"
        SELECT id, amount, date, description, project_id, responsible_id, tenant_id, created_at, updated_at
        FROM expenses
        WHERE id = $1 AND tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Expense not found".to_string()))?;

    let p = payload.expense;
    let mut errors = Vec::new();

    let description = p.description.map(|d| d.trim().to_string()).unwrap_or_else(|| existing.get("description"));
    if description.is_empty() {
        errors.push("Description can't be blank".to_string());
    }

    let amount: BigDecimal = p.amount.unwrap_or_else(|| existing.get("amount"));
    if amount <= BigDecimal::from(0) {
        errors.push("Amount must be greater than 0".to_string());
    }

    let date: NaiveDate = p.date.unwrap_or_else(|| existing.get("date"));
    let project_id: Option<Uuid> = p.project_id.or_else(|| existing.get("project_id"));
    let responsible_id: Uuid = p.responsible_id.unwrap_or_else(|| existing.get("responsible_id"));

    let mut project_summary: Option<ExpenseProjectSummary> = None;
    if let Some(pid) = project_id {
        let proj = sqlx::query(
            r#"SELECT id, name, status FROM projects WHERE id = $1 AND tenant_id = $2"#,
        )
        .bind(pid)
        .bind(user.tenant_id)
        .fetch_optional(&pool)
        .await
        .map_err(AppError::from)?;

        if let Some(proj) = proj {
            project_summary = Some(ExpenseProjectSummary {
                id: proj.get("id"),
                name: proj.get("name"),
                status: proj.get("status"),
            });
        }
    }

    if !errors.is_empty() {
        return Err(AppError::UnprocessableEntity(errors));
    }

    let now = Utc::now();

    let row = sqlx::query(
        r#"
        UPDATE expenses
        SET amount = $1, date = $2, description = $3, project_id = $4, responsible_id = $5, updated_at = $6
        WHERE id = $7 AND tenant_id = $8
        RETURNING id, amount, date, description, project_id, responsible_id, tenant_id, created_at, updated_at
        "#,
    )
    .bind(amount)
    .bind(date)
    .bind(description)
    .bind(project_id)
    .bind(responsible_id)
    .bind(now)
    .bind(id)
    .bind(user.tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    if let Some(receipt_val) = payload.receipt {
        handle_receipt_attachment(&pool, id, &receipt_val).await;
    }

    let res_amount: BigDecimal = row.get("amount");
    let res_date: NaiveDate = row.get("date");
    let created_at: DateTime<Utc> = row.get("created_at");
    let updated_at: DateTime<Utc> = row.get("updated_at");

    let resp = ExpenseWithProject {
        id: row.get("id"),
        amount: res_amount,
        date: res_date,
        description: row.get("description"),
        project_id: row.get("project_id"),
        responsible_id: row.get("responsible_id"),
        tenant_id: row.get("tenant_id"),
        created_at,
        updated_at,
        project: project_summary,
        receipt: None,
    };

    Ok(Json(resp))
}

pub async fn get_expense_receipt(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Query(query): Query<ReceiptQuery>,
) -> Result<Response, AppError> {
    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM expenses WHERE id = $1 AND tenant_id = $2)"#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    if !exists {
        return Err(AppError::NotFound("Expense not found".to_string()));
    }

    let attachment = StorageService::get_attachment(&pool, "Expense", id, "receipt").await?;
    let (blob, bytes) = attachment.ok_or_else(|| {
        AppError::NotFound("Comprovante não encontrado.".to_string())
    })?;

    let mut headers = HeaderMap::new();
    let content_type = blob.content_type.unwrap_or_else(|| "application/octet-stream".to_string());
    headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());

    let disposition_type = if query.disposition.as_deref() == Some("inline") {
        "inline"
    } else {
        "attachment"
    };

    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("{}; filename=\"{}\"", disposition_type, blob.filename).parse().unwrap(),
    );

    Ok((StatusCode::OK, headers, bytes).into_response())
}

async fn handle_receipt_attachment(pool: &PgPool, expense_id: Uuid, receipt_val: &serde_json::Value) {
    let (content_type, base64_data, filename) = match receipt_val {
        serde_json::Value::String(s) => {
            let ct = "image/jpeg";
            let (ct, clean_data) = if s.starts_with("data:") {
                if let Some(idx) = s.find(";base64,") {
                    let mime = &s[5..idx];
                    let data = &s[idx + 8..];
                    (mime, data)
                } else {
                    (ct, s.as_str())
                }
            } else {
                (ct, s.as_str())
            };
            (ct.to_string(), clean_data.to_string(), format!("comprovante_{}.jpg", Utc::now().timestamp()))
        }
        serde_json::Value::Object(map) => {
            let ct = map.get("mime_type").or_else(|| map.get("content_type")).and_then(|v| v.as_str()).unwrap_or("image/jpeg").to_string();
            let data_raw = map.get("data").and_then(|v| v.as_str()).unwrap_or("");
            let clean_data = if data_raw.starts_with("data:") {
                if let Some(idx) = data_raw.find(";base64,") {
                    &data_raw[idx + 8..]
                } else {
                    data_raw
                }
            } else {
                data_raw
            };
            let fn_name = map.get("filename").and_then(|v| v.as_str()).unwrap_or(&format!("comprovante_{}.jpg", Utc::now().timestamp())).to_string();
            (ct, clean_data.to_string(), fn_name)
        }
        _ => return,
    };

    if let Ok(bytes) = STANDARD.decode(&base64_data) {
        let _ = StorageService::save_attachment(
            pool,
            "Expense",
            expense_id,
            "receipt",
            &filename,
            &content_type,
            &bytes,
        )
        .await;
    }
}
