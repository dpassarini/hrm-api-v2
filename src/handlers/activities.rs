use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::AppError,
    models::activity::{
        ActivityContactSummary, ActivityWithContacts, CreateActivityParams, UpdateActivityParams,
    },
};

const VALID_STATUSES: &[&str] = &["pending", "completed", "cancelled"];

pub async fn create_project_activity(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<CreateActivityParams>,
) -> Result<impl IntoResponse, AppError> {
    let proj_exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1 AND tenant_id = $2)"#,
    )
    .bind(project_id)
    .bind(user.tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    if !proj_exists {
        return Err(AppError::NotFound("Project not found".to_string()));
    }

    let p = payload.activity;
    let mut errors = Vec::new();

    let title = p.title.trim().to_string();
    if title.is_empty() {
        errors.push("Title can't be blank".to_string());
    }

    let status = p.status.unwrap_or_else(|| "pending".to_string());
    if !VALID_STATUSES.contains(&status.as_str()) {
        errors.push("Status is not included in the list".to_string());
    }

    if !errors.is_empty() {
        return Err(AppError::UnprocessableEntity(errors));
    }

    let mut tx = pool.begin().await.map_err(AppError::from)?;
    let id = Uuid::new_v4();
    let now = Utc::now();

    let row = sqlx::query(
        r#"
        INSERT INTO activities (id, project_id, responsible_id, title, description, status, due_date, tenant_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
        RETURNING id, project_id, responsible_id, title, description, status, due_date, tenant_id, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(project_id)
    .bind(p.responsible_id)
    .bind(title)
    .bind(p.description)
    .bind(status)
    .bind(p.due_date)
    .bind(user.tenant_id)
    .bind(now)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::from)?;

    let act_id: Uuid = row.get("id");
    let mut contact_summaries = Vec::new();

    if let Some(contact_ids) = p.contact_ids {
        for cid in contact_ids {
            let ac_id = Uuid::new_v4();
            let _ = sqlx::query(
                r#"
                INSERT INTO activity_contacts (id, activity_id, contact_id, tenant_id, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $5)
                ON CONFLICT (activity_id, contact_id) DO NOTHING
                "#,
            )
            .bind(ac_id)
            .bind(act_id)
            .bind(cid)
            .bind(user.tenant_id)
            .bind(now)
            .execute(&mut *tx)
            .await;
        }

        let contacts_rows = sqlx::query(
            r#"
            SELECT c.id, c.name, c.email
            FROM contacts c
            JOIN activity_contacts ac ON ac.contact_id = c.id
            WHERE ac.activity_id = $1
            "#,
        )
        .bind(act_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(AppError::from)?;

        contact_summaries = contacts_rows
            .into_iter()
            .map(|c| ActivityContactSummary {
                id: c.get("id"),
                name: c.get("name"),
                email: c.get("email"),
            })
            .collect();
    }

    tx.commit().await.map_err(AppError::from)?;

    let act_created_at: DateTime<Utc> = row.get("created_at");
    let act_updated_at: DateTime<Utc> = row.get("updated_at");

    let resp = ActivityWithContacts {
        id: act_id,
        project_id: row.get("project_id"),
        responsible_id: row.get("responsible_id"),
        title: row.get("title"),
        description: row.get("description"),
        status: row.get("status"),
        due_date: row.get("due_date"),
        tenant_id: row.get("tenant_id"),
        created_at: act_created_at,
        updated_at: act_updated_at,
        contacts: contact_summaries,
    };

    Ok((StatusCode::CREATED, Json(resp)))
}

pub async fn update_activity(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateActivityParams>,
) -> Result<impl IntoResponse, AppError> {
    let existing = sqlx::query(
        r#"
        SELECT id, project_id, responsible_id, title, description, status, due_date, tenant_id
        FROM activities
        WHERE id = $1 AND tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Activity not found".to_string()))?;

    let p = payload.activity;
    let mut errors = Vec::new();

    let title = p.title.map(|t| t.trim().to_string()).unwrap_or_else(|| existing.get("title"));
    if title.is_empty() {
        errors.push("Title can't be blank".to_string());
    }

    let status: String = p.status.unwrap_or_else(|| existing.get("status"));
    if !VALID_STATUSES.contains(&status.as_str()) {
        errors.push("Status is not included in the list".to_string());
    }

    if !errors.is_empty() {
        return Err(AppError::UnprocessableEntity(errors));
    }

    let responsible_id: Uuid = p.responsible_id.unwrap_or_else(|| existing.get("responsible_id"));
    let description: Option<String> = p.description.or_else(|| existing.get("description"));
    let due_date: Option<DateTime<Utc>> = p.due_date.or_else(|| existing.get("due_date"));
    let now = Utc::now();

    let mut tx = pool.begin().await.map_err(AppError::from)?;

    let row = sqlx::query(
        r#"
        UPDATE activities
        SET title = $1, description = $2, responsible_id = $3, status = $4, due_date = $5, updated_at = $6
        WHERE id = $7 AND tenant_id = $8
        RETURNING id, project_id, responsible_id, title, description, status, due_date, tenant_id, created_at, updated_at
        "#,
    )
    .bind(title)
    .bind(description)
    .bind(responsible_id)
    .bind(status)
    .bind(due_date)
    .bind(now)
    .bind(id)
    .bind(user.tenant_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::from)?;

    if let Some(contact_ids) = p.contact_ids {
        let _ = sqlx::query(r#"DELETE FROM activity_contacts WHERE activity_id = $1"#)
            .bind(id)
            .execute(&mut *tx)
            .await;

        for cid in contact_ids {
            let ac_id = Uuid::new_v4();
            let _ = sqlx::query(
                r#"
                INSERT INTO activity_contacts (id, activity_id, contact_id, tenant_id, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $5)
                ON CONFLICT (activity_id, contact_id) DO NOTHING
                "#,
            )
            .bind(ac_id)
            .bind(id)
            .bind(cid)
            .bind(user.tenant_id)
            .bind(now)
            .execute(&mut *tx)
            .await;
        }
    }

    let contacts_rows = sqlx::query(
        r#"
        SELECT c.id, c.name, c.email
        FROM contacts c
        JOIN activity_contacts ac ON ac.contact_id = c.id
        WHERE ac.activity_id = $1
        "#,
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await
    .map_err(AppError::from)?;

    let contact_summaries = contacts_rows
        .into_iter()
        .map(|c| ActivityContactSummary {
            id: c.get("id"),
            name: c.get("name"),
            email: c.get("email"),
        })
        .collect();

    tx.commit().await.map_err(AppError::from)?;

    let act_created_at: DateTime<Utc> = row.get("created_at");
    let act_updated_at: DateTime<Utc> = row.get("updated_at");

    let resp = ActivityWithContacts {
        id: row.get("id"),
        project_id: row.get("project_id"),
        responsible_id: row.get("responsible_id"),
        title: row.get("title"),
        description: row.get("description"),
        status: row.get("status"),
        due_date: row.get("due_date"),
        tenant_id: row.get("tenant_id"),
        created_at: act_created_at,
        updated_at: act_updated_at,
        contacts: contact_summaries,
    };

    Ok(Json(resp))
}
