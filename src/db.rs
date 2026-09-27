use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;

use crate::{config::AppConfig, error::AppError};

pub async fn create_pool() -> Result<PgPool, AppError> {
    let config = AppConfig::get();
    
    let pool = PgPoolOptions::new()
        .max_connections(25)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(600))
        .connect(&config.database_url)
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to Postgres at {}: {}", config.database_url, e);
            AppError::InternalServerError(format!("Database connection failed: {}", e))
        })?;

    Ok(pool)
}
