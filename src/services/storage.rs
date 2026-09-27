use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use rand::{distributions::Alphanumeric, Rng};
use sqlx::{PgPool, Row};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::{config::AppConfig, error::AppError};

#[derive(Debug, sqlx::FromRow)]
pub struct BlobRecord {
    pub id: Uuid,
    pub key: String,
    pub filename: String,
    pub content_type: Option<String>,
    pub metadata: Option<String>,
    pub byte_size: i64,
    pub checksum: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

pub struct StorageService;

impl StorageService {
    fn generate_key() -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(28)
            .map(|c| (c as char).to_ascii_lowercase())
            .collect()
    }

    fn file_path(storage_dir: &str, key: &str) -> PathBuf {
        if key.len() >= 4 {
            let p1 = &key[0..2];
            let p2 = &key[2..4];
            Path::new(storage_dir).join(p1).join(p2).join(key)
        } else {
            Path::new(storage_dir).join(key)
        }
    }

    fn find_existing_file(key: &str) -> Option<PathBuf> {
        let config = AppConfig::get();
        let candidate1 = Self::file_path(&config.storage_dir, key);
        if candidate1.exists() {
            return Some(candidate1);
        }

        let candidate2 = Self::file_path("../hrm-api/storage", key);
        if candidate2.exists() {
            return Some(candidate2);
        }

        None
    }

    pub async fn save_attachment(
        pool: &PgPool,
        record_type: &str,
        record_id: Uuid,
        name: &str,
        filename: &str,
        content_type: &str,
        data: &[u8],
    ) -> Result<Uuid, AppError> {
        let config = AppConfig::get();
        let key = Self::generate_key();

        let digest = md5::compute(data);
        let checksum = STANDARD.encode(digest.0);

        let target_path = Self::file_path(&config.storage_dir, &key);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::InternalServerError(format!("Failed to create storage directory: {}", e))
            })?;
        }

        fs::write(&target_path, data).map_err(|e| {
            AppError::InternalServerError(format!("Failed to write file to storage: {}", e))
        })?;

        let blob_id = Uuid::new_v4();
        let now = Utc::now();
        let byte_size = data.len() as i64;
        let service_name = "local";

        sqlx::query(
            r#"
            INSERT INTO active_storage_blobs (id, key, filename, content_type, metadata, byte_size, checksum, created_at, service_name)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(blob_id)
        .bind(&key)
        .bind(filename)
        .bind(content_type)
        .bind("{}")
        .bind(byte_size)
        .bind(&checksum)
        .bind(now)
        .bind(service_name)
        .execute(pool)
        .await
        .map_err(AppError::from)?;

        let _ = sqlx::query(
            r#"
            DELETE FROM active_storage_attachments
            WHERE record_type = $1 AND record_id = $2 AND name = $3
            "#,
        )
        .bind(record_type)
        .bind(record_id)
        .bind(name)
        .execute(pool)
        .await;

        let attachment_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO active_storage_attachments (id, name, record_type, record_id, blob_id, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(attachment_id)
        .bind(name)
        .bind(record_type)
        .bind(record_id)
        .bind(blob_id)
        .bind(now)
        .execute(pool)
        .await
        .map_err(AppError::from)?;

        Ok(blob_id)
    }

    pub async fn get_attachment(
        pool: &PgPool,
        record_type: &str,
        record_id: Uuid,
        name: &str,
    ) -> Result<Option<(BlobRecord, Vec<u8>)>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT b.id, b.key, b.filename, b.content_type, b.metadata, b.byte_size, b.checksum, b.created_at
            FROM active_storage_blobs b
            JOIN active_storage_attachments a ON a.blob_id = b.id
            WHERE a.record_type = $1 AND a.record_id = $2 AND a.name = $3
            LIMIT 1
            "#,
        )
        .bind(record_type)
        .bind(record_id)
        .bind(name)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?;

        let row = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let key: String = row.get("key");
        let file_path = Self::find_existing_file(&key).ok_or_else(|| {
            AppError::NotFound(format!("Physical file for key '{}' not found on disk", key))
        })?;

        let bytes = fs::read(&file_path).map_err(|e| {
            AppError::InternalServerError(format!("Failed to read attachment file: {}", e))
        })?;

        let blob = BlobRecord {
            id: row.get("id"),
            key,
            filename: row.get("filename"),
            content_type: row.get("content_type"),
            metadata: row.get("metadata"),
            byte_size: row.get("byte_size"),
            checksum: row.get("checksum"),
            created_at: row.get("created_at"),
        };

        Ok(Some((blob, bytes)))
    }
}
