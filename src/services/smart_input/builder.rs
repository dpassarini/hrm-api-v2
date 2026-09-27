use base64::{engine::general_purpose::STANDARD, Engine as _};
use bigdecimal::{BigDecimal, FromPrimitive};
use chrono::{NaiveDate, Utc};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::AppError,
    models::{company::Company, project::Project},
    services::storage::StorageService,
};

pub struct EntityBuilderService;

impl EntityBuilderService {
    pub async fn commit(
        pool: &PgPool,
        payload: &Value,
        user: &AuthUser,
    ) -> Result<String, AppError> {
        let tenant_id = user.tenant_id;
        let responsible_id = user.user_uuid().unwrap_or(tenant_id);

        let mut tx = pool.begin().await.map_err(AppError::from)?;
        let now = Utc::now();

        let mut company_records: HashMap<String, Company> = HashMap::new();
        let mut project_records: HashMap<String, Project> = HashMap::new();

        // 1. Create Companies
        if let Some(companies_arr) = payload.get("companies").and_then(|v| v.as_array()) {
            for comp_val in companies_arr {
                if let Some(comp_name) = comp_val.get("name").and_then(|v| v.as_str()).map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    let mut categories = Vec::new();
                    if let Some(cats_arr) = comp_val.get("categories").and_then(|v| v.as_array()) {
                        for c in cats_arr {
                            if let Some(cat_name) = c.as_str().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                                categories.push(cat_name.to_string());
                                let cat_id = Uuid::new_v4();
                                let _ = sqlx::query(
                                    r#"
                                    INSERT INTO categories (id, name, tenant_id, created_at, updated_at)
                                    VALUES ($1, $2, $3, $4, $4)
                                    ON CONFLICT (tenant_id, name) DO NOTHING
                                    "#,
                                )
                                .bind(cat_id)
                                .bind(cat_name)
                                .bind(tenant_id)
                                .bind(now)
                                .execute(&mut *tx)
                                .await;
                            }
                        }
                    }

                    let existing = sqlx::query_as::<_, Company>(
                        r#"
                        SELECT id, name, cnpj, website, categories, tenant_id, created_at, updated_at
                        FROM companies
                        WHERE tenant_id = $1 AND name = $2
                        LIMIT 1
                        "#,
                    )
                    .bind(tenant_id)
                    .bind(comp_name)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(AppError::from)?;

                    let comp = match existing {
                        Some(mut c) => {
                            if !categories.is_empty() {
                                let mut merged = c.categories.unwrap_or_default();
                                for cat in &categories {
                                    if !merged.contains(cat) {
                                        merged.push(cat.clone());
                                    }
                                }
                                let _ = sqlx::query(
                                    r#"
                                    UPDATE companies
                                    SET categories = $1, updated_at = $2
                                    WHERE id = $3
                                    "#,
                                )
                                .bind(&merged)
                                .bind(now)
                                .bind(c.id)
                                .execute(&mut *tx)
                                .await;
                                c.categories = Some(merged);
                            }
                            c
                        }
                        None => {
                            let comp_id = Uuid::new_v4();
                            let cats_opt = if categories.is_empty() { None } else { Some(categories) };
                            sqlx::query_as::<_, Company>(
                                r#"
                                INSERT INTO companies (id, name, categories, tenant_id, created_at, updated_at)
                                VALUES ($1, $2, $3, $4, $5, $5)
                                RETURNING id, name, cnpj, website, categories, tenant_id, created_at, updated_at
                                "#,
                            )
                            .bind(comp_id)
                            .bind(comp_name)
                            .bind(cats_opt)
                            .bind(tenant_id)
                            .bind(now)
                            .fetch_one(&mut *tx)
                            .await
                            .map_err(AppError::from)?
                        }
                    };

                    company_records.insert(comp_name.to_string(), comp);
                }
            }
        }

        // 2. Create Contacts
        if let Some(contacts_arr) = payload.get("contacts").and_then(|v| v.as_array()) {
            for cont_val in contacts_arr {
                if let Some(name) = cont_val.get("name").and_then(|v| v.as_str()).map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    let email = cont_val.get("email").and_then(|v| v.as_str()).map(|s| s.trim().to_string());
                    let phone = cont_val.get("phone").and_then(|v| v.as_str()).map(|s| s.trim().to_string());
                    let job_title = cont_val.get("job_title").and_then(|v| v.as_str()).map(|s| s.trim().to_string());

                    let mut company_id: Option<Uuid> = None;
                    if let Some(comp_name) = cont_val.get("company_name").and_then(|v| v.as_str()).map(|s| s.trim()).filter(|s| !s.is_empty()) {
                        if let Some(c) = company_records.get(comp_name) {
                            company_id = Some(c.id);
                        } else {
                            let existing = sqlx::query_as::<_, Company>(
                                r#"
                                SELECT id, name, cnpj, website, categories, tenant_id, created_at, updated_at
                                FROM companies
                                WHERE tenant_id = $1 AND name = $2
                                LIMIT 1
                                "#,
                            )
                            .bind(tenant_id)
                            .bind(comp_name)
                            .fetch_optional(&mut *tx)
                            .await
                            .map_err(AppError::from)?;

                            let comp = match existing {
                                Some(c) => c,
                                None => {
                                    let comp_id = Uuid::new_v4();
                                    let empty_cats: Vec<String> = vec!["Geral".to_string()];
                                    sqlx::query_as::<_, Company>(
                                        r#"
                                        INSERT INTO companies (id, name, categories, tenant_id, created_at, updated_at)
                                        VALUES ($1, $2, $3, $4, $5, $5)
                                        RETURNING id, name, cnpj, website, categories, tenant_id, created_at, updated_at
                                        "#,
                                    )
                                    .bind(comp_id)
                                    .bind(comp_name)
                                    .bind(&empty_cats)
                                    .bind(tenant_id)
                                    .bind(now)
                                    .fetch_one(&mut *tx)
                                    .await
                                    .map_err(AppError::from)?
                                }
                            };
                            company_id = Some(comp.id);
                            company_records.insert(comp_name.to_string(), comp);
                        }
                    }

                    if let Some(cid) = company_id {
                        let contact_id = Uuid::new_v4();
                        let _ = sqlx::query(
                            r#"
                            INSERT INTO contacts (id, company_id, name, email, phone, job_title, tenant_id, created_at, updated_at)
                            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
                            "#,
                        )
                        .bind(contact_id)
                        .bind(cid)
                        .bind(name)
                        .bind(email)
                        .bind(phone)
                        .bind(job_title)
                        .bind(tenant_id)
                        .bind(now)
                        .execute(&mut *tx)
                        .await
                        .map_err(AppError::from)?;
                    }
                }
            }
        }

        // 3. Create Projects
        if let Some(projects_arr) = payload.get("projects").and_then(|v| v.as_array()) {
            for proj_val in projects_arr {
                if let Some(proj_name) = proj_val.get("name").and_then(|v| v.as_str()).map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    let status = proj_val.get("status").and_then(|v| v.as_str()).unwrap_or("planned");
                    let proj_id = Uuid::new_v4();

                    let project = sqlx::query_as::<_, Project>(
                        r#"
                        INSERT INTO projects (id, name, status, responsible_id, tenant_id, created_at, updated_at)
                        VALUES ($1, $2, $3, $4, $5, $6, $6)
                        RETURNING id, name, description, responsible_id, status, start_date, end_date, tenant_id, created_at, updated_at
                        "#,
                    )
                    .bind(proj_id)
                    .bind(proj_name)
                    .bind(status)
                    .bind(responsible_id)
                    .bind(tenant_id)
                    .bind(now)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(AppError::from)?;

                    project_records.insert(proj_name.to_string(), project);
                }
            }
        }

        // 4. Create Activities
        if let Some(activities_arr) = payload.get("activities").and_then(|v| v.as_array()) {
            for act_val in activities_arr {
                if let Some(title) = act_val.get("title").and_then(|v| v.as_str()).map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    let status = act_val.get("status").and_then(|v| v.as_str()).unwrap_or("pending");
                    let due_date = act_val.get("due_date").and_then(|v| v.as_str()).and_then(|d| {
                        NaiveDate::parse_from_str(d, "%Y-%m-%d")
                            .ok()
                            .and_then(|nd| nd.and_hms_opt(12, 0, 0))
                            .map(|ndt| ndt.and_utc())
                    });

                    let mut project_id: Option<Uuid> = None;
                    if let Some(proj_name) = act_val.get("project_name").and_then(|v| v.as_str()).map(|s| s.trim()).filter(|s| !s.is_empty()) {
                        if let Some(p) = project_records.get(proj_name) {
                            project_id = Some(p.id);
                        } else {
                            let p_opt = sqlx::query_as::<_, Project>(
                                r#"
                                SELECT id, name, description, responsible_id, status, start_date, end_date, tenant_id, created_at, updated_at
                                FROM projects
                                WHERE tenant_id = $1 AND name = $2
                                LIMIT 1
                                "#,
                            )
                            .bind(tenant_id)
                            .bind(proj_name)
                            .fetch_optional(&mut *tx)
                            .await
                            .map_err(AppError::from)?;

                            if let Some(p) = p_opt {
                                project_id = Some(p.id);
                                project_records.insert(proj_name.to_string(), p);
                            }
                        }
                    }

                    let act_proj_id = match project_id {
                        Some(pid) => pid,
                        None => {
                            let default_proj_name = "Geral";
                            let p_opt = sqlx::query_as::<_, Project>(
                                r#"
                                SELECT id, name, description, responsible_id, status, start_date, end_date, tenant_id, created_at, updated_at
                                FROM projects
                                WHERE tenant_id = $1 AND name = $2
                                LIMIT 1
                                "#,
                            )
                            .bind(tenant_id)
                            .bind(default_proj_name)
                            .fetch_optional(&mut *tx)
                            .await
                            .map_err(AppError::from)?;

                            match p_opt {
                                Some(p) => p.id,
                                None => {
                                    let pid = Uuid::new_v4();
                                    let p = sqlx::query_as::<_, Project>(
                                        r#"
                                        INSERT INTO projects (id, name, status, responsible_id, tenant_id, created_at, updated_at)
                                        VALUES ($1, $2, 'planned', $3, $4, $5, $5)
                                        RETURNING id, name, description, responsible_id, status, start_date, end_date, tenant_id, created_at, updated_at
                                        "#,
                                    )
                                    .bind(pid)
                                    .bind(default_proj_name)
                                    .bind(responsible_id)
                                    .bind(tenant_id)
                                    .bind(now)
                                    .fetch_one(&mut *tx)
                                    .await
                                    .map_err(AppError::from)?;
                                    p.id
                                }
                            }
                        }
                    };

                    let act_id = Uuid::new_v4();
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO activities (id, project_id, responsible_id, title, status, due_date, tenant_id, created_at, updated_at)
                        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
                        "#,
                    )
                    .bind(act_id)
                    .bind(act_proj_id)
                    .bind(responsible_id)
                    .bind(title)
                    .bind(status)
                    .bind(due_date)
                    .bind(tenant_id)
                    .bind(now)
                    .execute(&mut *tx)
                    .await
                    .map_err(AppError::from)?;
                }
            }
        }

        // 5. Create Expenses & attach receipts
        let mut created_expense_ids = Vec::new();
        if let Some(expenses_arr) = payload.get("expenses").and_then(|v| v.as_array()) {
            for exp_val in expenses_arr {
                let amount = exp_val.get("amount").and_then(|v| {
                    if let Some(num) = v.as_f64() {
                        BigDecimal::from_f64(num)
                    } else if let Some(s) = v.as_str() {
                        BigDecimal::from_str(s).ok()
                    } else {
                        None
                    }
                }).unwrap_or_else(|| BigDecimal::from(0));

                if amount <= BigDecimal::from(0) {
                    continue;
                }

                let mut desc = exp_val.get("description").and_then(|v| v.as_str()).unwrap_or("Despesa importada via Smart Input").to_string();
                if let Some(est) = exp_val.get("establishment").and_then(|v| v.as_str()).map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    if !desc.to_lowercase().contains(&est.to_lowercase()) {
                        desc = format!("{} ({})", desc, est);
                    }
                }

                let exp_date = exp_val.get("date").and_then(|v| v.as_str()).and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).unwrap_or_else(|| now.date_naive());

                let mut project_id: Option<Uuid> = None;
                if let Some(pid_str) = exp_val.get("project_id").and_then(|v| v.as_str()) {
                    project_id = Uuid::parse_str(pid_str).ok();
                } else if let Some(proj_name) = exp_val.get("project_name").and_then(|v| v.as_str()).map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    if let Some(p) = project_records.get(proj_name) {
                        project_id = Some(p.id);
                    } else {
                        let p_opt = sqlx::query_as::<_, Project>(
                            r#"
                            SELECT id, name, description, responsible_id, status, start_date, end_date, tenant_id, created_at, updated_at
                            FROM projects
                            WHERE tenant_id = $1 AND name = $2
                            LIMIT 1
                            "#,
                        )
                        .bind(tenant_id)
                        .bind(proj_name)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(AppError::from)?;

                        if let Some(p) = p_opt {
                            project_id = Some(p.id);
                        }
                    }
                }

                let expense_id = Uuid::new_v4();
                let _ = sqlx::query(
                    r#"
                    INSERT INTO expenses (id, amount, date, description, project_id, responsible_id, tenant_id, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
                    "#,
                )
                .bind(expense_id)
                .bind(amount)
                .bind(exp_date)
                .bind(desc)
                .bind(project_id)
                .bind(responsible_id)
                .bind(tenant_id)
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(AppError::from)?;

                created_expense_ids.push((expense_id, exp_val.clone()));
            }
        }

        tx.commit().await.map_err(AppError::from)?;

        for (expense_id, exp_val) in created_expense_ids {
            let receipt_val = exp_val.get("receipt_image")
                .or_else(|| payload.get("receipt_image"))
                .or_else(|| payload.get("attachment"));

            if let Some(receipt) = receipt_val {
                Self::try_attach_receipt(pool, expense_id, receipt).await;
            }
        }

        Ok("Entidades importadas com sucesso para o HRM".to_string())
    }

    async fn try_attach_receipt(pool: &PgPool, expense_id: Uuid, receipt: &Value) {
        let (content_type, base64_data, filename) = match receipt {
            Value::String(s) => {
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
                (ct.to_string(), clean_data.to_string(), format!("comprovante_smart_input_{}.jpg", Utc::now().timestamp()))
            }
            Value::Object(map) => {
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
                let fn_name = map.get("filename").and_then(|v| v.as_str()).unwrap_or(&format!("comprovante_smart_input_{}.jpg", Utc::now().timestamp())).to_string();
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
}
