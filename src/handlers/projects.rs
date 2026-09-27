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
        activity::{ActivityContactSummary, ActivityWithContacts},
        expense::Expense,
        pagination::PaginationMeta,
        project::{
            CreateProjectParams, Project, ProjectFilterQuery, ProjectWithDetails,
            ProjectsResponse, UpdateProjectParams,
        },
    },
};

const VALID_STATUSES: &[&str] = &["planned", "in_progress", "completed", "cancelled"];

pub async fn list_projects(
    State(pool): State<PgPool>,
    user: AuthUser,
    Query(query): Query<ProjectFilterQuery>,
) -> Result<impl IntoResponse, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).max(1).min(100);
    let offset = (page - 1) * per_page;

    let name_filter = query.name.as_deref().map(|n| format!("%{}%", n.trim()));
    let status_filter = query.status.as_deref().map(|s| s.trim().to_string());

    let total_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM projects
        WHERE tenant_id = $1
          AND ($2::text IS NULL OR name ILIKE $2)
          AND ($3::text IS NULL OR status = $3)
          AND ($4::uuid IS NULL OR responsible_id = $4)
        "#,
    )
    .bind(user.tenant_id)
    .bind(name_filter.as_deref())
    .bind(status_filter.as_deref())
    .bind(query.responsible_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    let projects = sqlx::query_as::<_, Project>(
        r#"
        SELECT id, name, description, responsible_id, status, start_date, end_date, tenant_id, created_at, updated_at
        FROM projects
        WHERE tenant_id = $1
          AND ($2::text IS NULL OR name ILIKE $2)
          AND ($3::text IS NULL OR status = $3)
          AND ($4::uuid IS NULL OR responsible_id = $4)
        ORDER BY created_at DESC
        LIMIT $5 OFFSET $6
        "#,
    )
    .bind(user.tenant_id)
    .bind(name_filter.as_deref())
    .bind(status_filter.as_deref())
    .bind(query.responsible_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let meta = PaginationMeta::new(total_count, per_page);
    Ok(Json(ProjectsResponse { projects, meta }))
}

pub async fn get_project(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let project = sqlx::query_as::<_, Project>(
        r#"
        SELECT id, name, description, responsible_id, status, start_date, end_date, tenant_id, created_at, updated_at
        FROM projects
        WHERE id = $1 AND tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Project not found".to_string()))?;

    let acts = sqlx::query(
        r#"
        SELECT id, project_id, responsible_id, title, description, status, due_date, tenant_id, created_at, updated_at
        FROM activities
        WHERE project_id = $1 AND tenant_id = $2
        ORDER BY created_at ASC
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let mut activities = Vec::new();
    for a in acts {
        let act_id: Uuid = a.get("id");
        let contacts_rows = sqlx::query(
            r#"
            SELECT c.id, c.name, c.email
            FROM contacts c
            JOIN activity_contacts ac ON ac.contact_id = c.id
            WHERE ac.activity_id = $1
            "#,
        )
        .bind(act_id)
        .fetch_all(&pool)
        .await
        .map_err(AppError::from)?;

        let contacts = contacts_rows
            .into_iter()
            .map(|c| ActivityContactSummary {
                id: c.get("id"),
                name: c.get("name"),
                email: c.get("email"),
            })
            .collect();

        activities.push(ActivityWithContacts {
            id: act_id,
            project_id: a.get("project_id"),
            responsible_id: a.get("responsible_id"),
            title: a.get("title"),
            description: a.get("description"),
            status: a.get("status"),
            due_date: a.get("due_date"),
            tenant_id: a.get("tenant_id"),
            created_at: a.get("created_at"),
            updated_at: a.get("updated_at"),
            contacts,
        });
    }

    let expenses = sqlx::query_as::<_, Expense>(
        r#"
        SELECT id, amount, date, description, project_id, responsible_id, tenant_id, created_at, updated_at
        FROM expenses
        WHERE project_id = $1 AND tenant_id = $2
        ORDER BY date DESC
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let detail = ProjectWithDetails {
        id: project.id,
        name: project.name,
        description: project.description,
        responsible_id: project.responsible_id,
        status: project.status,
        start_date: project.start_date,
        end_date: project.end_date,
        tenant_id: project.tenant_id,
        created_at: project.created_at,
        updated_at: project.updated_at,
        activities,
        expenses,
    };

    Ok(Json(detail))
}

pub async fn create_project(
    State(pool): State<PgPool>,
    user: AuthUser,
    Json(payload): Json<CreateProjectParams>,
) -> Result<impl IntoResponse, AppError> {
    let p = payload.project;
    let mut errors = Vec::new();

    let name = p.name.trim().to_string();
    if name.is_empty() {
        errors.push("Name can't be blank".to_string());
    }

    let status = p.status.unwrap_or_else(|| "planned".to_string());
    if !VALID_STATUSES.contains(&status.as_str()) {
        errors.push("Status is not included in the list".to_string());
    }

    if !errors.is_empty() {
        return Err(AppError::UnprocessableEntity(errors));
    }

    let id = Uuid::new_v4();
    let now = Utc::now();

    let project = sqlx::query_as::<_, Project>(
        r#"
        INSERT INTO projects (id, name, description, responsible_id, status, start_date, end_date, tenant_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
        RETURNING id, name, description, responsible_id, status, start_date, end_date, tenant_id, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(p.description)
    .bind(p.responsible_id)
    .bind(status)
    .bind(p.start_date)
    .bind(p.end_date)
    .bind(user.tenant_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    Ok((StatusCode::CREATED, Json(project)))
}

pub async fn update_project(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateProjectParams>,
) -> Result<impl IntoResponse, AppError> {
    let existing = sqlx::query_as::<_, Project>(
        r#"
        SELECT id, name, description, responsible_id, status, start_date, end_date, tenant_id, created_at, updated_at
        FROM projects
        WHERE id = $1 AND tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Project not found".to_string()))?;

    let p = payload.project;
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

    let description = p.description.or(existing.description);
    let responsible_id = p.responsible_id.unwrap_or(existing.responsible_id);
    let start_date = p.start_date.or(existing.start_date);
    let end_date = p.end_date.or(existing.end_date);
    let now = Utc::now();

    let updated = sqlx::query_as::<_, Project>(
        r#"
        UPDATE projects
        SET name = $1, description = $2, responsible_id = $3, status = $4,
            start_date = $5, end_date = $6, updated_at = $7
        WHERE id = $8 AND tenant_id = $9
        RETURNING id, name, description, responsible_id, status, start_date, end_date, tenant_id, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(description)
    .bind(responsible_id)
    .bind(status)
    .bind(start_date)
    .bind(end_date)
    .bind(now)
    .bind(id)
    .bind(user.tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    Ok(Json(updated))
}
