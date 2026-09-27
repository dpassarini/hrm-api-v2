use axum::{
    extract::{Path, State},
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
    models::category::{
        Category, CreateCategoryParams, UpdateCategoryParams,
    },
};

pub async fn list_categories(
    State(pool): State<PgPool>,
    user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let categories = sqlx::query_as::<_, Category>(
        r#"
        SELECT id, name, tenant_id, created_at, updated_at
        FROM categories
        WHERE tenant_id = $1 OR tenant_id IS NULL
        ORDER BY name ASC
        "#,
    )
    .bind(user.tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    Ok(Json(categories))
}

pub async fn get_category(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let category = sqlx::query_as::<_, Category>(
        r#"
        SELECT id, name, tenant_id, created_at, updated_at
        FROM categories
        WHERE id = $1 AND (tenant_id = $2 OR tenant_id IS NULL)
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Category not found".to_string()))?;

    Ok(Json(category))
}

pub async fn create_category(
    State(pool): State<PgPool>,
    user: AuthUser,
    Json(payload): Json<CreateCategoryParams>,
) -> Result<impl IntoResponse, AppError> {
    if !user.is_admin() && !user.is_app_client() {
        return Err(AppError::Forbidden(
            "Only administrators can manage categories.".to_string(),
        ));
    }

    let p = payload.category;
    let name = p.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::UnprocessableEntity(vec![
            "Name can't be blank".to_string(),
        ]));
    }

    let is_global_req = match p.global {
        Some(serde_json::Value::Bool(b)) => b,
        Some(serde_json::Value::String(ref s)) => s == "true",
        _ => false,
    };

    let target_tenant_id = if is_global_req && user.is_super_admin() {
        None
    } else {
        Some(user.tenant_id)
    };

    let exists: bool = if let Some(tid) = target_tenant_id {
        sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM categories WHERE tenant_id = $1 AND LOWER(name) = LOWER($2))"#,
        )
        .bind(tid)
        .bind(&name)
        .fetch_one(&pool)
        .await
        .map_err(AppError::from)?
    } else {
        sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM categories WHERE tenant_id IS NULL AND LOWER(name) = LOWER($1))"#,
        )
        .bind(&name)
        .fetch_one(&pool)
        .await
        .map_err(AppError::from)?
    };

    if exists {
        return Err(AppError::UnprocessableEntity(vec![
            "Name has already been taken".to_string(),
        ]));
    }

    let id = Uuid::new_v4();
    let now = Utc::now();

    let category = sqlx::query_as::<_, Category>(
        r#"
        INSERT INTO categories (id, name, tenant_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4)
        RETURNING id, name, tenant_id, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(target_tenant_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    Ok((StatusCode::CREATED, Json(category)))
}

pub async fn update_category(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateCategoryParams>,
) -> Result<impl IntoResponse, AppError> {
    if !user.is_admin() && !user.is_app_client() {
        return Err(AppError::Forbidden(
            "Only administrators can manage categories.".to_string(),
        ));
    }

    let existing = sqlx::query_as::<_, Category>(
        r#"
        SELECT id, name, tenant_id, created_at, updated_at
        FROM categories
        WHERE id = $1 AND (tenant_id = $2 OR tenant_id IS NULL)
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Category not found".to_string()))?;

    if existing.tenant_id.is_none() && !user.is_super_admin() {
        return Err(AppError::Forbidden(
            "Only super administrators can edit global categories.".to_string(),
        ));
    }

    let new_name = payload.category.name.trim().to_string();
    if new_name.is_empty() {
        return Err(AppError::UnprocessableEntity(vec![
            "Name can't be blank".to_string(),
        ]));
    }

    let old_name = existing.name.unwrap_or_default();
    let mut tx = pool.begin().await.map_err(AppError::from)?;
    let now = Utc::now();

    let updated = sqlx::query_as::<_, Category>(
        r#"
        UPDATE categories
        SET name = $1, updated_at = $2
        WHERE id = $3
        RETURNING id, name, tenant_id, created_at, updated_at
        "#,
    )
    .bind(new_name.clone())
    .bind(now)
    .bind(id)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::from)?;

    if !old_name.is_empty() && old_name != new_name {
        if existing.tenant_id.is_none() {
            let _ = sqlx::query(
                r#"
                UPDATE companies
                SET categories = array_replace(categories, $1, $2), updated_at = $3
                WHERE $1 = ANY(categories)
                "#,
            )
            .bind(&old_name)
            .bind(&new_name)
            .bind(now)
            .execute(&mut *tx)
            .await;
        } else {
            let _ = sqlx::query(
                r#"
                UPDATE companies
                SET categories = array_replace(categories, $1, $2), updated_at = $3
                WHERE tenant_id = $4 AND $1 = ANY(categories)
                "#,
            )
            .bind(&old_name)
            .bind(&new_name)
            .bind(now)
            .bind(existing.tenant_id)
            .execute(&mut *tx)
            .await;
        }
    }

    tx.commit().await.map_err(AppError::from)?;

    Ok(Json(updated))
}
