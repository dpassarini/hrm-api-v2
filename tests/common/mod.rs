#![allow(dead_code)]

use hrm_api_v2::auth::Claims;
use jsonwebtoken::{encode, EncodingKey, Header};
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::RsaPrivateKey;
use std::sync::OnceLock;
use uuid::Uuid;

pub struct TestKeys {
    pub private_pem: String,
    pub public_pem: String,
}

static TEST_KEYS: OnceLock<TestKeys> = OnceLock::new();

pub fn get_test_keys() -> &'static TestKeys {
    TEST_KEYS.get_or_init(|| {
        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("failed to generate test RSA key");
        let public_key = rsa::RsaPublicKey::from(&private_key);

        let private_pem = private_key
            .to_pkcs8_pem(LineEnding::LF)
            .expect("failed to encode private key")
            .to_string();

        let public_pem = public_key
            .to_public_key_pem(LineEnding::LF)
            .expect("failed to encode public key");

        // Set env variable for AppConfig
        std::env::set_var("JWT_PUBLIC_KEY", &public_pem);

        if let Ok(dec_key) = jsonwebtoken::DecodingKey::from_rsa_pem(public_pem.as_bytes()) {
            hrm_api_v2::auth::set_custom_decoding_key(dec_key);
        }

        TestKeys {
            private_pem,
            public_pem,
        }
    })
}


pub fn create_test_token(
    user_id: &str,
    tenant_id: Option<&str>,
    profile: Option<&str>,
    email: Option<&str>,
    name: Option<&str>,
    expired: bool,
    custom_iss: Option<&str>,
    custom_aud: Option<serde_json::Value>,
) -> String {
    let keys = get_test_keys();
    let encoding_key = EncodingKey::from_rsa_pem(keys.private_pem.as_bytes()).expect("invalid test private key");

    let now = chrono::Utc::now().timestamp() as usize;
    let exp = if expired {
        now - 3600
    } else {
        now + 3600
    };

    let claims = Claims {
        sub: user_id.to_string(),
        tenant_id: tenant_id.map(|s| s.to_string()),
        email: email.map(|s| s.to_string()),
        name: name.map(|s| s.to_string()),
        profile: profile.map(|s| s.to_string()),
        iss: Some(custom_iss.unwrap_or("unified_login").to_string()),
        aud: Some(custom_aud.unwrap_or_else(|| serde_json::json!(["hrm-api"]))),
        exp: Some(exp),
    };

    let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("test-key".to_string());

    encode(&header, &claims, &encoding_key).expect("failed to encode test JWT")
}

pub fn create_valid_token(user_id: Uuid, tenant_id: Uuid, profile: &str) -> String {
    create_test_token(
        &user_id.to_string(),
        Some(&tenant_id.to_string()),
        Some(profile),
        Some("user@test.com"),
        Some("Test User"),
        false,
        None,
        None,
    )
}

pub async fn get_test_pool() -> Option<sqlx::PgPool> {
    let _ = get_test_keys();
    let pool = hrm_api_v2::db::create_pool().await.ok()?;

    let ddl_statements = [
        r#"CREATE TABLE IF NOT EXISTS categories (
            id UUID PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            tenant_id UUID,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT uq_categories_tenant_name UNIQUE (tenant_id, name)
        )"#,
        r#"CREATE TABLE IF NOT EXISTS companies (
            id UUID PRIMARY KEY,
            name VARCHAR(255),
            cnpj VARCHAR(255),
            website VARCHAR(255),
            categories TEXT[],
            tenant_id UUID NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
        r#"CREATE TABLE IF NOT EXISTS contacts (
            id UUID PRIMARY KEY,
            company_id UUID NOT NULL,
            name VARCHAR(255) NOT NULL,
            email VARCHAR(255),
            phone VARCHAR(255),
            job_title VARCHAR(255),
            tenant_id UUID NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
        r#"CREATE TABLE IF NOT EXISTS projects (
            id UUID PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            description TEXT,
            responsible_id UUID,
            status VARCHAR(50) DEFAULT 'planned',
            start_date DATE,
            end_date DATE,
            tenant_id UUID NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
        r#"CREATE TABLE IF NOT EXISTS activities (
            id UUID PRIMARY KEY,
            project_id UUID NOT NULL,
            responsible_id UUID,
            title VARCHAR(255) NOT NULL,
            description TEXT,
            status VARCHAR(50) DEFAULT 'pending',
            due_date TIMESTAMPTZ,
            tenant_id UUID NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
        r#"CREATE TABLE IF NOT EXISTS expenses (
            id UUID PRIMARY KEY,
            amount NUMERIC(12, 2) NOT NULL,
            date DATE NOT NULL,
            description VARCHAR(255) NOT NULL,
            project_id UUID,
            responsible_id UUID,
            tenant_id UUID NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
        r#"CREATE TABLE IF NOT EXISTS leads (
            id UUID PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            company_name VARCHAR(255),
            email VARCHAR(255),
            phone VARCHAR(255),
            job_title VARCHAR(255),
            website VARCHAR(255),
            city VARCHAR(255),
            state VARCHAR(255),
            source VARCHAR(255),
            status VARCHAR(50) NOT NULL DEFAULT 'pending',
            notes TEXT,
            categories TEXT[],
            responsible_id UUID,
            tenant_id UUID NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
        r#"CREATE TABLE IF NOT EXISTS leads_files (
            id UUID PRIMARY KEY,
            filename VARCHAR(255),
            status VARCHAR(50),
            records_count INTEGER DEFAULT 0,
            processed_count INTEGER DEFAULT 0,
            responsible_id UUID,
            tenant_id UUID NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
        r#"CREATE TABLE IF NOT EXISTS leads_file_lines (
            id UUID PRIMARY KEY,
            leads_file_id UUID NOT NULL,
            line_number INTEGER,
            data TEXT,
            status INTEGER NOT NULL DEFAULT 0,
            error_message TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
        r#"CREATE TABLE IF NOT EXISTS active_storage_blobs (
            id UUID PRIMARY KEY,
            key VARCHAR(255) UNIQUE NOT NULL,
            filename VARCHAR(255) NOT NULL,
            content_type VARCHAR(255),
            metadata TEXT,
            byte_size BIGINT NOT NULL,
            checksum VARCHAR(255),
            service_name VARCHAR(255) NOT NULL DEFAULT 'local',
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
        r#"CREATE TABLE IF NOT EXISTS active_storage_attachments (
            id UUID PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            record_type VARCHAR(255) NOT NULL,
            record_id UUID NOT NULL,
            blob_id UUID NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
    ];

    for stmt in ddl_statements {
        let _ = sqlx::query(stmt).execute(&pool).await;
    }

    let alter_statements = [
        "ALTER TABLE categories ALTER COLUMN created_at TYPE TIMESTAMPTZ, ALTER COLUMN updated_at TYPE TIMESTAMPTZ;",
        "ALTER TABLE companies ALTER COLUMN created_at TYPE TIMESTAMPTZ, ALTER COLUMN updated_at TYPE TIMESTAMPTZ;",
        "ALTER TABLE contacts ALTER COLUMN created_at TYPE TIMESTAMPTZ, ALTER COLUMN updated_at TYPE TIMESTAMPTZ;",
        "ALTER TABLE projects ALTER COLUMN created_at TYPE TIMESTAMPTZ, ALTER COLUMN updated_at TYPE TIMESTAMPTZ;",
        "ALTER TABLE activities ALTER COLUMN created_at TYPE TIMESTAMPTZ, ALTER COLUMN updated_at TYPE TIMESTAMPTZ, ALTER COLUMN due_date TYPE TIMESTAMPTZ;",
        "ALTER TABLE expenses ALTER COLUMN created_at TYPE TIMESTAMPTZ, ALTER COLUMN updated_at TYPE TIMESTAMPTZ;",
        "ALTER TABLE leads ALTER COLUMN created_at TYPE TIMESTAMPTZ, ALTER COLUMN updated_at TYPE TIMESTAMPTZ;",
        "ALTER TABLE leads_files ALTER COLUMN created_at TYPE TIMESTAMPTZ, ALTER COLUMN updated_at TYPE TIMESTAMPTZ;",
        "ALTER TABLE leads_file_lines ALTER COLUMN created_at TYPE TIMESTAMPTZ, ALTER COLUMN updated_at TYPE TIMESTAMPTZ;",
        "ALTER TABLE active_storage_blobs ALTER COLUMN created_at TYPE TIMESTAMPTZ;",
        "ALTER TABLE active_storage_attachments ALTER COLUMN created_at TYPE TIMESTAMPTZ;",
    ];

    for stmt in alter_statements {
        let _ = sqlx::query(stmt).execute(&pool).await;
    }

    Some(pool)
}


pub async fn setup_test_app() -> Option<(axum::Router, sqlx::PgPool)> {
    let pool = get_test_pool().await?;
    let app = hrm_api_v2::create_app(pool.clone());
    Some((app, pool))
}


