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
        company::{
            CompaniesResponse, Company, CompanyFilterQuery, CreateCompanyParams,
            UpdateCompanyParams,
        },
        pagination::PaginationMeta,
    },
};

pub async fn list_companies(
    State(pool): State<PgPool>,
    user: AuthUser,
    Query(query): Query<CompanyFilterQuery>,
) -> Result<impl IntoResponse, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).max(1).min(100);
    let offset = (page - 1) * per_page;

    let name_filter = query.name.as_deref().map(|n| format!("%{}%", n.trim()));
    let cat_filter = query.category.as_deref().map(|c| c.trim().to_string());

    let total_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM companies
        WHERE tenant_id = $1
          AND ($2::text IS NULL OR name ILIKE $2)
          AND ($3::text IS NULL OR $3 = ANY(categories))
        "#,
    )
    .bind(user.tenant_id)
    .bind(name_filter.as_deref())
    .bind(cat_filter.as_deref())
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    let companies = sqlx::query_as::<_, Company>(
        r#"
        SELECT id, name, cnpj, website, categories, tenant_id, created_at, updated_at
        FROM companies
        WHERE tenant_id = $1
          AND ($2::text IS NULL OR name ILIKE $2)
          AND ($3::text IS NULL OR $3 = ANY(categories))
        ORDER BY created_at DESC
        LIMIT $4 OFFSET $5
        "#,
    )
    .bind(user.tenant_id)
    .bind(name_filter.as_deref())
    .bind(cat_filter.as_deref())
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let meta = PaginationMeta::new(total_count, per_page);
    Ok(Json(CompaniesResponse { companies, meta }))
}

pub async fn get_company(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let company = sqlx::query_as::<_, Company>(
        r#"
        SELECT id, name, cnpj, website, categories, tenant_id, created_at, updated_at
        FROM companies
        WHERE id = $1 AND tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Company not found".to_string()))?;

    Ok(Json(company))
}

pub async fn create_company(
    State(pool): State<PgPool>,
    user: AuthUser,
    Json(payload): Json<CreateCompanyParams>,
) -> Result<impl IntoResponse, AppError> {
    let p = payload.company;
    let mut errors = Vec::new();

    let name = p.name.trim().to_string();
    if name.is_empty() {
        errors.push("Name can't be blank".to_string());
    }

    let raw_categories = p.categories.unwrap_or_default();
    let categories: Vec<String> = raw_categories
        .into_iter()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect();

    if categories.is_empty() {
        errors.push("Categories must have at least one category".to_string());
    } else {
        let valid_rows = sqlx::query(
            r#"
            SELECT name
            FROM categories
            WHERE (tenant_id = $1 OR tenant_id IS NULL) AND name = ANY($2)
            "#,
        )
        .bind(user.tenant_id)
        .bind(&categories)
        .fetch_all(&pool)
        .await
        .map_err(AppError::from)?;

        let valid_categories: Vec<String> = valid_rows
            .into_iter()
            .filter_map(|r| r.get::<Option<String>, _>("name"))
            .collect();

        let invalid: Vec<String> = categories
            .iter()
            .filter(|c| !valid_categories.iter().any(|vc| vc.eq_ignore_ascii_case(c)))
            .cloned()
            .collect();

        if !invalid.is_empty() {
            errors.push(format!(
                "Categories must be pre-registered (invalid: {})",
                invalid.join(", ")
            ));
        }
    }

    if !errors.is_empty() {
        return Err(AppError::UnprocessableEntity(errors));
    }

    let id = Uuid::new_v4();
    let now = Utc::now();

    let company = sqlx::query_as::<_, Company>(
        r#"
        INSERT INTO companies (id, name, cnpj, website, categories, tenant_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
        RETURNING id, name, cnpj, website, categories, tenant_id, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(p.cnpj)
    .bind(p.website)
    .bind(&categories)
    .bind(user.tenant_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    Ok((StatusCode::CREATED, Json(company)))
}

pub async fn update_company(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateCompanyParams>,
) -> Result<impl IntoResponse, AppError> {
    let existing = sqlx::query_as::<_, Company>(
        r#"
        SELECT id, name, cnpj, website, categories, tenant_id, created_at, updated_at
        FROM companies
        WHERE id = $1 AND tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Company not found".to_string()))?;

    let p = payload.company;
    let mut errors = Vec::new();

    let name = p.name.map(|n| n.trim().to_string()).unwrap_or_else(|| existing.name.unwrap_or_default());
    if name.is_empty() {
        errors.push("Name can't be blank".to_string());
    }

    let categories = if let Some(raw_cats) = p.categories {
        let cleaned: Vec<String> = raw_cats
            .into_iter()
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect();

        if cleaned.is_empty() {
            errors.push("Categories must have at least one category".to_string());
            cleaned
        } else {
            let valid_rows = sqlx::query(
                r#"
                SELECT name
                FROM categories
                WHERE (tenant_id = $1 OR tenant_id IS NULL) AND name = ANY($2)
                "#,
            )
            .bind(user.tenant_id)
            .bind(&cleaned)
            .fetch_all(&pool)
            .await
            .map_err(AppError::from)?;

            let valid_categories: Vec<String> = valid_rows
                .into_iter()
                .filter_map(|r| r.get::<Option<String>, _>("name"))
                .collect();

            let invalid: Vec<String> = cleaned
                .iter()
                .filter(|c| !valid_categories.iter().any(|vc| vc.eq_ignore_ascii_case(c)))
                .cloned()
                .collect();

            if !invalid.is_empty() {
                errors.push(format!(
                    "Categories must be pre-registered (invalid: {})",
                    invalid.join(", ")
                ));
            }
            cleaned
        }
    } else {
        existing.categories.unwrap_or_default()
    };

    if !errors.is_empty() {
        return Err(AppError::UnprocessableEntity(errors));
    }

    let cnpj = p.cnpj.or(existing.cnpj);
    let website = p.website.or(existing.website);
    let now = Utc::now();

    let updated = sqlx::query_as::<_, Company>(
        r#"
        UPDATE companies
        SET name = $1, cnpj = $2, website = $3, categories = $4, updated_at = $5
        WHERE id = $6 AND tenant_id = $7
        RETURNING id, name, cnpj, website, categories, tenant_id, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(cnpj)
    .bind(website)
    .bind(&categories)
    .bind(now)
    .bind(id)
    .bind(user.tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    Ok(Json(updated))
}
