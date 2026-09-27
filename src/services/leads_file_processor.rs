use chrono::Utc;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{error::AppError, models::lead::Lead};

pub struct LeadsFileProcessor;

impl LeadsFileProcessor {
    pub async fn process_file_in_background(
        pool: PgPool,
        leads_file_id: Uuid,
        tenant_id: Uuid,
        file_bytes: Vec<u8>,
    ) {
        tokio::spawn(async move {
            if let Err(e) = Self::do_process_file(&pool, leads_file_id, tenant_id, &file_bytes).await {
                tracing::error!("[LeadsFileProcessor] Error processing file {}: {}", leads_file_id, e);
            }
        });
    }

    pub async fn do_process_file(
        pool: &PgPool,
        leads_file_id: Uuid,
        tenant_id: Uuid,
        file_bytes: &[u8],
    ) -> Result<(), AppError> {
        let content_str = match String::from_utf8(file_bytes.to_vec()) {
            Ok(s) => s,
            Err(_) => String::from_utf8_lossy(file_bytes).to_string(),
        };

        let clean_content = content_str.strip_prefix('\u{feff}').unwrap_or(&content_str);
        let first_line = clean_content.lines().next().unwrap_or("");
        let delimiter = if first_line.matches(';').count() > first_line.matches(',').count() {
            b';'
        } else {
            b','
        };

        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .flexible(true)
            .from_reader(clean_content.as_bytes());

        let headers = rdr
            .headers()
            .map_err(|e| AppError::UnprocessableEntityMsg(format!("CSV header error: {}", e)))?
            .clone();

        let header_names: Vec<String> = headers
            .iter()
            .map(|h| h.trim().to_lowercase())
            .collect();

        let mut line_ids = Vec::new();

        for result in rdr.records() {
            let record = match result {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("Skipping malformed CSV line: {}", e);
                    continue;
                }
            };

            let mut map = HashMap::new();
            let mut all_blank = true;

            for (idx, field) in record.iter().enumerate() {
                if let Some(header) = header_names.get(idx) {
                    let val = field.trim().to_string();
                    if !val.is_empty() {
                        all_blank = false;
                        map.insert(header.clone(), serde_json::Value::String(val));
                    }
                }
            }

            if all_blank {
                continue;
            }

            let line_id = Uuid::new_v4();
            let now = Utc::now();
            let data_json = serde_json::to_value(&map).unwrap_or(serde_json::json!({}));

            let _ = sqlx::query(
                r#"
                INSERT INTO leads_file_lines (id, leads_file_id, status, data, created_at, updated_at)
                VALUES ($1, $2, 0, $3, $4, $4)
                "#,
            )
            .bind(line_id)
            .bind(leads_file_id)
            .bind(data_json)
            .bind(now)
            .execute(pool)
            .await;

            line_ids.push((line_id, map));
        }

        for (line_id, map) in line_ids {
            Self::process_line(pool, line_id, tenant_id, map).await;
        }

        Ok(())
    }

    async fn process_line(
        pool: &PgPool,
        line_id: Uuid,
        tenant_id: Uuid,
        data: HashMap<String, serde_json::Value>,
    ) {
        let now = Utc::now();

        let _ = sqlx::query(
            r#"
            UPDATE leads_file_lines
            SET status = 1, updated_at = $1
            WHERE id = $2
            "#,
        )
        .bind(now)
        .bind(line_id)
        .execute(pool)
        .await;

        let get_val = |keys: &[&str]| -> Option<String> {
            for k in keys {
                if let Some(serde_json::Value::String(s)) = data.get(*k) {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
            None
        };

        let name = get_val(&["nome", "name", "contato", "contact"]).unwrap_or_else(|| "Sem Nome".to_string());
        let company_name = get_val(&["empresa", "company", "company_name", "nome_empresa"]);
        let email = get_val(&["email", "e-mail"]);
        let phone = get_val(&["telefone", "phone", "celular", "whatsapp"]);
        let job_title = get_val(&["cargo", "job_title", "funcao", "posicao"]);
        let website = get_val(&["site", "website", "url"]);
        let city = get_val(&["cidade", "city"]);
        let state = get_val(&["estado", "state", "uf"]);
        let notes = get_val(&["observacoes", "notes", "obs", "anotacoes"]);

        let mut categories: Vec<String> = Vec::new();
        if let Some(cat_raw) = get_val(&["categoria", "categorias", "setor", "category", "categories"]) {
            for part in cat_raw.split(&[',', ';', '|'][..]) {
                let trimmed = part.trim();
                if !trimmed.is_empty() && !categories.contains(&trimmed.to_string()) {
                    categories.push(trimmed.to_string());
                }
            }
        }

        for cat_name in &categories {
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
            .execute(pool)
            .await;
        }

        let lead_id = Uuid::new_v4();
        let cats_opt = if categories.is_empty() { None } else { Some(categories) };

        let insert_res = sqlx::query_as::<_, Lead>(
            r#"
            INSERT INTO leads (
                id, name, company_name, email, phone, job_title, website, city, state,
                source, status, notes, categories, tenant_id, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'importacao_arquivo', 'new', $10, $11, $12, $13, $13)
            RETURNING id, name, company_name, email, phone, job_title, website, city, state,
                      source, status, notes, categories, responsible_id, tenant_id, created_at, updated_at
            "#,
        )
        .bind(lead_id)
        .bind(name)
        .bind(company_name)
        .bind(email)
        .bind(phone)
        .bind(job_title)
        .bind(website)
        .bind(city)
        .bind(state)
        .bind(notes)
        .bind(cats_opt)
        .bind(tenant_id)
        .bind(now)
        .fetch_one(pool)
        .await;

        let end_now = Utc::now();
        match insert_res {
            Ok(_) => {
                let _ = sqlx::query(
                    r#"
                    UPDATE leads_file_lines
                    SET status = 3, error_message = NULL, updated_at = $1
                    WHERE id = $2
                    "#,
                )
                .bind(end_now)
                .bind(line_id)
                .execute(pool)
                .await;
            }
            Err(e) => {
                let err_msg = e.to_string();
                let _ = sqlx::query(
                    r#"
                    UPDATE leads_file_lines
                    SET status = 2, error_message = $1, updated_at = $2
                    WHERE id = $3
                    "#,
                )
                .bind(err_msg)
                .bind(end_now)
                .bind(line_id)
                .execute(pool)
                .await;
            }
        }
    }
}
