use axum::{
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::AppError,
    models::{
        leads_file::{
            CreateLeadsFileResponse, LeadsFileDetail, LeadsFileErrorLine, LeadsFileSummary,
            LeadsFilesResponse,
        },
        pagination::{PaginationMeta, PaginationQuery},
    },
    services::{leads_file_processor::LeadsFileProcessor, storage::StorageService},
};

pub async fn list_leads_files(
    State(pool): State<PgPool>,
    user: AuthUser,
    Query(query): Query<PaginationQuery>,
) -> Result<impl IntoResponse, AppError> {
    let per_page = query.per_page();
    let offset = query.offset();

    let total_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM leads_files WHERE tenant_id = $1"#,
    )
    .bind(user.tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    let files = sqlx::query(
        r#"
        SELECT lf.id, lf.created_at, b.filename, b.byte_size
        FROM leads_files lf
        LEFT JOIN active_storage_attachments a ON a.record_type = 'LeadsFile' AND a.record_id = lf.id AND a.name = 'file'
        LEFT JOIN active_storage_blobs b ON b.id = a.blob_id
        WHERE lf.tenant_id = $1
        ORDER BY lf.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user.tenant_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let mut leads_files = Vec::new();

    for f in files {
        let f_id: Uuid = f.get("id");
        let counts = sqlx::query(
            r#"
            SELECT status, COUNT(*) as cnt
            FROM leads_file_lines
            WHERE leads_file_id = $1
            GROUP BY status
            "#,
        )
        .bind(f_id)
        .fetch_all(&pool)
        .await
        .map_err(AppError::from)?;

        let mut success_count = 0i64;
        let mut error_count = 0i64;
        let mut processing_count = 0i64;
        let mut total_lines = 0i64;

        for c in counts {
            let status: i32 = c.get("status");
            let cnt: i64 = c.get("cnt");
            total_lines += cnt;
            match status {
                3 => success_count += cnt,
                2 => error_count += cnt,
                0 | 1 => processing_count += cnt,
                _ => {}
            }
        }

        let filename: Option<String> = f.get("filename");
        let byte_size: Option<i64> = f.get("byte_size");
        let created_at: DateTime<Utc> = f.get("created_at");

        leads_files.push(LeadsFileSummary {
            id: f_id,
            filename: filename.unwrap_or_else(|| "Sem arquivo".to_string()),
            byte_size: byte_size.unwrap_or(0),
            created_at,
            total_lines,
            success_count,
            error_count,
            processing_count,
        });
    }

    let meta = PaginationMeta::new(total_count, per_page);
    Ok(Json(LeadsFilesResponse { leads_files, meta }))
}

pub async fn get_leads_file(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let file = sqlx::query(
        r#"
        SELECT lf.id, lf.created_at, b.filename
        FROM leads_files lf
        LEFT JOIN active_storage_attachments a ON a.record_type = 'LeadsFile' AND a.record_id = lf.id AND a.name = 'file'
        LEFT JOIN active_storage_blobs b ON b.id = a.blob_id
        WHERE lf.id = $1 AND lf.tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::NotFound("Leads file not found".to_string()))?;

    let counts = sqlx::query(
        r#"
        SELECT status, COUNT(*) as cnt
        FROM leads_file_lines
        WHERE leads_file_id = $1
        GROUP BY status
        "#,
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let mut success_count = 0i64;
    let mut error_count = 0i64;
    let mut processing_count = 0i64;
    let mut total_lines = 0i64;

    for c in counts {
        let status: i32 = c.get("status");
        let cnt: i64 = c.get("cnt");
        total_lines += cnt;
        match status {
            3 => success_count += cnt,
            2 => error_count += cnt,
            0 | 1 => processing_count += cnt,
            _ => {}
        }
    }

    let error_rows = sqlx::query(
        r#"
        SELECT id, data, error_message, created_at
        FROM leads_file_lines
        WHERE leads_file_id = $1 AND status = 2
        ORDER BY created_at ASC
        LIMIT 100
        "#,
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let error_lines = error_rows
        .into_iter()
        .map(|r| LeadsFileErrorLine {
            id: r.get("id"),
            data: r.get("data"),
            error_message: r.get("error_message"),
            created_at: r.get("created_at"),
        })
        .collect();

    let filename: Option<String> = file.get("filename");
    let created_at: DateTime<Utc> = file.get("created_at");

    let detail = LeadsFileDetail {
        id: file.get("id"),
        filename: filename.unwrap_or_else(|| "Sem arquivo".to_string()),
        created_at,
        total_lines,
        success_count,
        error_count,
        processing_count,
        error_lines,
    };

    Ok(Json(detail))
}

pub async fn create_leads_file(
    State(pool): State<PgPool>,
    user: AuthUser,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut original_filename = "leads_import.csv".to_string();
    let mut mime_type = "text/csv".to_string();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            if let Some(fn_str) = field.file_name() {
                original_filename = fn_str.to_string();
            }
            if let Some(ct) = field.content_type() {
                mime_type = ct.to_string();
            }
            let data = field.bytes().await.map_err(|e| {
                AppError::UnprocessableEntityMsg(format!("Failed to read uploaded file: {}", e))
            })?;
            file_bytes = Some(data.to_vec());
        }
    }

    let bytes = file_bytes.ok_or_else(|| {
        AppError::UnprocessableEntityMsg("Arquivo não fornecido".to_string())
    })?;

    let file_id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO leads_files (id, tenant_id, created_at, updated_at)
        VALUES ($1, $2, $3, $3)
        "#,
    )
    .bind(file_id)
    .bind(user.tenant_id)
    .bind(now)
    .execute(&pool)
    .await
    .map_err(AppError::from)?;

    // Save attachment to ActiveStorage
    StorageService::save_attachment(
        &pool,
        "LeadsFile",
        file_id,
        "file",
        &original_filename,
        &mime_type,
        &bytes,
    )
    .await?;

    // Spawn background job
    LeadsFileProcessor::process_file_in_background(
        pool.clone(),
        file_id,
        user.tenant_id,
        bytes,
    )
    .await;

    Ok((
        StatusCode::ACCEPTED,
        Json(CreateLeadsFileResponse {
            message: "Arquivo recebido com sucesso. O processamento iniciou em segundo plano.".to_string(),
            id: file_id,
        }),
    ))
}

pub async fn download_leads_file(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM leads_files WHERE id = $1 AND tenant_id = $2)"#,
    )
    .bind(id)
    .bind(user.tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    if !exists {
        return Err(AppError::NotFound("Leads file not found".to_string()));
    }

    let attachment = StorageService::get_attachment(&pool, "LeadsFile", id, "file").await?;
    let (blob, bytes) = attachment.ok_or_else(|| {
        AppError::NotFound("Arquivo não encontrado.".to_string())
    })?;

    let mut headers = HeaderMap::new();
    let content_type = blob.content_type.unwrap_or_else(|| "text/csv".to_string());
    headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", blob.filename).parse().unwrap(),
    );

    Ok((StatusCode::OK, headers, bytes).into_response())
}
