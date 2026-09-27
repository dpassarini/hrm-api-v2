use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::AppError,
    models::{
        contact::{
            CompanySummary, ContactFilterQuery, ContactWithCompany, ContactsResponse,
            CreateContactParams, UpdateContactParams,
        },
        pagination::PaginationMeta,
    },
};

pub async fn list_contacts(
    State(pool): State<PgPool>,
    user: AuthUser,
    Query(query): Query<ContactFilterQuery>,
) -> Result<impl IntoResponse, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).max(1).min(100);
    let offset = (page - 1) * per_page;

    let name_filter = query.name.as_deref().map(|n| format!("%{}%", n.trim()));
    let email_filter = query.email.as_deref().map(|e| format!("%{}%", e.trim()));

    let total_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM contacts
        WHERE tenant_id = $1
          AND ($2::text IS NULL OR name ILIKE $2)
          AND ($3::text IS NULL OR email ILIKE $3)
        "#,
    )
    .bind(user.tenant_id)
    .bind(name_filter.as_deref())
    .bind(email_filter.as_deref())
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    let rows = sqlx::query(
        r#"
        SELECT c.id, c.company_id, c.name, c.email, c.phone, c.job_title, c.tenant_id,
               c.created_at, c.updated_at, comp.id as comp_id, comp.name as comp_name
        FROM contacts c
        LEFT JOIN companies comp ON comp.id = c.company_id
        WHERE c.tenant_id = $1
          AND ($2::text IS NULL OR c.name ILIKE $2)
          AND ($3::text IS NULL OR c.email ILIKE $3)
        ORDER BY c.created_at DESC
        LIMIT $4 OFFSET $5
        "#,
    )
    .bind(user.tenant_id)
    .bind(name_filter.as_deref())
    .bind(email_filter.as_deref())
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let contacts = rows
        .into_iter()
        .map(|r| {
            let comp_id: Option<Uuid> = r.get("comp_id");
            let comp_name: Option<String> = r.get("comp_name");
            ContactWithCompany {
                id: r.get("id"),
                company_id: r.get("company_id"),
                name: r.get("name"),
                email: r.get("email"),
                phone: r.get("phone"),
                job_title: r.get("job_title"),
                tenant_id: r.get("tenant_id"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                company: comp_id.map(|cid| CompanySummary {
                    id: cid,
                    name: comp_name,
                }),
            }
        })
        .collect();

    let meta = PaginationMeta::new(total_count, per_page);
    Ok(Json(ContactsResponse { contacts, meta }))
}

pub async fn get_contact(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let r = sqlx::query(
        r#"
        SELECT c.id, c.company_id, c.name, c.email, c.phone, c.job_title, c.tenant_id,
               c.created_at, c.updated_at, comp.id as comp_id, comp.name as comp_name
        FROM contacts c
        LEFT JOIN companies comp ON comp.id = c.company_id
        WHERE c.id = $1 AND c.tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Contact not found".to_string()))?;

    let comp_id: Option<Uuid> = r.get("comp_id");
    let comp_name: Option<String> = r.get("comp_name");

    let contact = ContactWithCompany {
        id: r.get("id"),
        company_id: r.get("company_id"),
        name: r.get("name"),
        email: r.get("email"),
        phone: r.get("phone"),
        job_title: r.get("job_title"),
        tenant_id: r.get("tenant_id"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        company: comp_id.map(|cid| CompanySummary {
            id: cid,
            name: comp_name,
        }),
    };

    Ok(Json(contact))
}

pub async fn create_contact(
    State(pool): State<PgPool>,
    user: AuthUser,
    Json(payload): Json<CreateContactParams>,
) -> Result<impl IntoResponse, AppError> {
    let p = payload.contact;
    let mut errors = Vec::new();

    let name = p.name.trim().to_string();
    if name.is_empty() {
        errors.push("Name can't be blank".to_string());
    }

    let comp_exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM companies WHERE id = $1 AND tenant_id = $2)"#,
    )
    .bind(p.company_id)
    .bind(user.tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    if !comp_exists {
        errors.push("Company must exist".to_string());
    }

    if !errors.is_empty() {
        return Err(AppError::UnprocessableEntity(errors));
    }

    let id = Uuid::new_v4();
    let now = Utc::now();

    let r = sqlx::query(
        r#"
        INSERT INTO contacts (id, company_id, name, email, phone, job_title, tenant_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
        RETURNING id, company_id, name, email, phone, job_title, tenant_id, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(p.company_id)
    .bind(name)
    .bind(p.email)
    .bind(p.phone)
    .bind(p.job_title)
    .bind(user.tenant_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    let comp_row = sqlx::query(r#"SELECT id, name FROM companies WHERE id = $1"#)
        .bind(p.company_id)
        .fetch_optional(&pool)
        .await
        .map_err(AppError::from)?;

    let resp = ContactWithCompany {
        id: r.get("id"),
        company_id: r.get("company_id"),
        name: r.get("name"),
        email: r.get("email"),
        phone: r.get("phone"),
        job_title: r.get("job_title"),
        tenant_id: r.get("tenant_id"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        company: comp_row.map(|c| CompanySummary {
            id: c.get("id"),
            name: c.get("name"),
        }),
    };

    Ok((StatusCode::CREATED, Json(resp)))
}

pub async fn update_contact(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateContactParams>,
) -> Result<impl IntoResponse, AppError> {
    let existing = sqlx::query(
        r#"
        SELECT id, company_id, name, email, phone, job_title, tenant_id
        FROM contacts
        WHERE id = $1 AND tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Contact not found".to_string()))?;

    let p = payload.contact;
    let mut errors = Vec::new();

    let name = p.name.map(|n| n.trim().to_string()).unwrap_or_else(|| existing.get("name"));
    if name.is_empty() {
        errors.push("Name can't be blank".to_string());
    }

    let company_id: Uuid = p.company_id.unwrap_or_else(|| existing.get("company_id"));
    let comp_exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM companies WHERE id = $1 AND tenant_id = $2)"#,
    )
    .bind(company_id)
    .bind(user.tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    if !comp_exists {
        errors.push("Company must exist".to_string());
    }

    if !errors.is_empty() {
        return Err(AppError::UnprocessableEntity(errors));
    }

    let email: Option<String> = p.email.or_else(|| existing.get("email"));
    let phone: Option<String> = p.phone.or_else(|| existing.get("phone"));
    let job_title: Option<String> = p.job_title.or_else(|| existing.get("job_title"));
    let now = Utc::now();

    let updated = sqlx::query(
        r#"
        UPDATE contacts
        SET company_id = $1, name = $2, email = $3, phone = $4, job_title = $5, updated_at = $6
        WHERE id = $7 AND tenant_id = $8
        RETURNING id, company_id, name, email, phone, job_title, tenant_id, created_at, updated_at
        "#,
    )
    .bind(company_id)
    .bind(name)
    .bind(email)
    .bind(phone)
    .bind(job_title)
    .bind(now)
    .bind(id)
    .bind(user.tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    let comp_row = sqlx::query(r#"SELECT id, name FROM companies WHERE id = $1"#)
        .bind(company_id)
        .fetch_optional(&pool)
        .await
        .map_err(AppError::from)?;

    let resp = ContactWithCompany {
        id: updated.get("id"),
        company_id: updated.get("company_id"),
        name: updated.get("name"),
        email: updated.get("email"),
        phone: updated.get("phone"),
        job_title: updated.get("job_title"),
        tenant_id: updated.get("tenant_id"),
        created_at: updated.get("created_at"),
        updated_at: updated.get("updated_at"),
        company: comp_row.map(|c| CompanySummary {
            id: c.get("id"),
            name: c.get("name"),
        }),
    };

    Ok(Json(resp))
}
