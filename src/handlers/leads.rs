use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::AppError,
    models::{
        lead::{
            ConvertLeadResponse, CreateLeadParams, Lead, LeadFilterQuery, LeadsResponse,
            UpdateLeadParams,
        },
        pagination::PaginationMeta,
    },
    services::lead_conversion::LeadConversionService,
};

const VALID_STATUSES: &[&str] = &["new", "contacted", "qualified", "converted", "lost"];

pub async fn list_leads(
    State(pool): State<PgPool>,
    user: AuthUser,
    Query(query): Query<LeadFilterQuery>,
) -> Result<impl IntoResponse, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).max(1).min(100);
    let offset = (page - 1) * per_page;

    let name_filter = query.name.as_deref().map(|n| format!("%{}%", n.trim()));
    let comp_filter = query.company_name.as_deref().map(|c| format!("%{}%", c.trim()));
    let status_filter = query.status.as_deref().map(|s| s.trim().to_string());
    let cat_filter = query.category.as_deref().map(|c| c.trim().to_string());

    let total_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM leads
        WHERE tenant_id = $1
          AND ($2::text IS NULL OR name ILIKE $2)
          AND ($3::text IS NULL OR company_name ILIKE $3)
          AND ($4::text IS NULL OR status = $4)
          AND ($5::text IS NULL OR $5 = ANY(categories))
        "#,
    )
    .bind(user.tenant_id)
    .bind(name_filter.as_deref())
    .bind(comp_filter.as_deref())
    .bind(status_filter.as_deref())
    .bind(cat_filter.as_deref())
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    let leads = sqlx::query_as::<_, Lead>(
        r#"
        SELECT id, name, company_name, email, phone, job_title, website, city, state,
               source, status, notes, categories, responsible_id, tenant_id, created_at, updated_at
        FROM leads
        WHERE tenant_id = $1
          AND ($2::text IS NULL OR name ILIKE $2)
          AND ($3::text IS NULL OR company_name ILIKE $3)
          AND ($4::text IS NULL OR status = $4)
          AND ($5::text IS NULL OR $5 = ANY(categories))
        ORDER BY created_at DESC
        LIMIT $6 OFFSET $7
        "#,
    )
    .bind(user.tenant_id)
    .bind(name_filter.as_deref())
    .bind(comp_filter.as_deref())
    .bind(status_filter.as_deref())
    .bind(cat_filter.as_deref())
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let meta = PaginationMeta::new(total_count, per_page);
    Ok(Json(LeadsResponse { leads, meta }))
}

pub async fn get_lead(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let lead = sqlx::query_as::<_, Lead>(
        r#"
        SELECT id, name, company_name, email, phone, job_title, website, city, state,
               source, status, notes, categories, responsible_id, tenant_id, created_at, updated_at
        FROM leads
        WHERE id = $1 AND tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Lead not found".to_string()))?;

    Ok(Json(lead))
}

pub async fn create_lead(
    State(pool): State<PgPool>,
    user: AuthUser,
    Json(payload): Json<CreateLeadParams>,
) -> Result<impl IntoResponse, AppError> {
    let p = payload.lead;
    let mut errors = Vec::new();

    let name = p.name.trim().to_string();
    if name.is_empty() {
        errors.push("Name can't be blank".to_string());
    }

    let status = p.status.unwrap_or_else(|| "new".to_string());
    if !VALID_STATUSES.contains(&status.as_str()) {
        errors.push("Status is not included in the list".to_string());
    }

    if !errors.is_empty() {
        return Err(AppError::UnprocessableEntity(errors));
    }

    let id = Uuid::new_v4();
    let now = Utc::now();
    let source = p.source.unwrap_or_else(|| "manual".to_string());

    let lead = sqlx::query_as::<_, Lead>(
        r#"
        INSERT INTO leads (
            id, name, company_name, email, phone, job_title, website, city, state,
            source, status, notes, categories, responsible_id, tenant_id, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $16)
        RETURNING id, name, company_name, email, phone, job_title, website, city, state,
                  source, status, notes, categories, responsible_id, tenant_id, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(p.company_name)
    .bind(p.email)
    .bind(p.phone)
    .bind(p.job_title)
    .bind(p.website)
    .bind(p.city)
    .bind(p.state)
    .bind(source)
    .bind(status)
    .bind(p.notes)
    .bind(p.categories)
    .bind(p.responsible_id)
    .bind(user.tenant_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    Ok((StatusCode::CREATED, Json(lead)))
}

pub async fn update_lead(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateLeadParams>,
) -> Result<impl IntoResponse, AppError> {
    let existing = sqlx::query_as::<_, Lead>(
        r#"
        SELECT id, name, company_name, email, phone, job_title, website, city, state,
               source, status, notes, categories, responsible_id, tenant_id, created_at, updated_at
        FROM leads
        WHERE id = $1 AND tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Lead not found".to_string()))?;

    let p = payload.lead;
    let mut errors = Vec::new();

    let name = p.name.map(|n| n.trim().to_string()).unwrap_or(existing.name);
    if name.is_empty() {
        errors.push("Name can't be blank".to_string());
    }

    let status = p.status.unwrap_or(existing.status);
    if !VALID_STATUSES.contains(&status.as_str()) {
        errors.push("Status is not included in the list".to_string());
    }

    if !errors.is_empty() {
        return Err(AppError::UnprocessableEntity(errors));
    }

    let company_name = p.company_name.or(existing.company_name);
    let email = p.email.or(existing.email);
    let phone = p.phone.or(existing.phone);
    let job_title = p.job_title.or(existing.job_title);
    let website = p.website.or(existing.website);
    let city = p.city.or(existing.city);
    let state = p.state.or(existing.state);
    let source = p.source.or(existing.source);
    let notes = p.notes.or(existing.notes);
    let categories = p.categories.or(existing.categories);
    let responsible_id = p.responsible_id.or(existing.responsible_id);
    let now = Utc::now();

    let updated = sqlx::query_as::<_, Lead>(
        r#"
        UPDATE leads
        SET name = $1, company_name = $2, email = $3, phone = $4, job_title = $5,
            website = $6, city = $7, state = $8, source = $9, status = $10,
            notes = $11, categories = $12, responsible_id = $13, updated_at = $14
        WHERE id = $15 AND tenant_id = $16
        RETURNING id, name, company_name, email, phone, job_title, website, city, state,
                  source, status, notes, categories, responsible_id, tenant_id, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(company_name)
    .bind(email)
    .bind(phone)
    .bind(job_title)
    .bind(website)
    .bind(city)
    .bind(state)
    .bind(source)
    .bind(status)
    .bind(notes)
    .bind(categories)
    .bind(responsible_id)
    .bind(now)
    .bind(id)
    .bind(user.tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    Ok(Json(updated))
}

pub async fn delete_lead(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let rows_affected = sqlx::query(r#"DELETE FROM leads WHERE id = $1 AND tenant_id = $2"#)
        .bind(id)
        .bind(user.tenant_id)
        .execute(&pool)
        .await
        .map_err(AppError::from)?
        .rows_affected();

    if rows_affected == 0 {
        return Err(AppError::NotFound("Lead not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn convert_lead(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let (company, contact, lead) =
        LeadConversionService::convert(&pool, id, user.tenant_id).await?;

    Ok(Json(ConvertLeadResponse {
        message: "Lead convertido com sucesso em Empresa e Contato".to_string(),
        company,
        contact,
        lead,
    }))
}
