    #![allow(unused_imports, dead_code)]

    use crate::create_app;
    use crate::auth::*;
    use crate::config::*;
    use crate::db::*;
    use crate::error::*;

    use crate::models::{
        activity::*, category::*, company::*, contact::*, dashboard::*, expense::*, lead::*,
        leads_file::*, pagination::*, project::*,
    };
    use crate::services::{
        lead_conversion::*, leads_file_processor::*, smart_input::{builder::*, deepseek::*, extractor::*, gemini::*}, storage::*,
    };
    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
    };
    use bigdecimal::BigDecimal;
    use chrono::{NaiveDate, Utc};
    use http_body_util::BodyExt;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
    use rsa::RsaPrivateKey;
    use serde_json::json;
    use std::str::FromStr;
    use std::sync::OnceLock;
    use tower::ServiceExt;
    use uuid::Uuid;

    struct TestEnv {
        pub private_pem: String,
        pub public_pem: String,
    }

    static TEST_ENV: OnceLock<TestEnv> = OnceLock::new();

    fn get_test_env() -> &'static TestEnv {
        TEST_ENV.get_or_init(|| {
            let mut rng = rand::thread_rng();
            let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("RSA gen error");
            let public_key = rsa::RsaPublicKey::from(&private_key);

            let private_pem = private_key
                .to_pkcs8_pem(LineEnding::LF)
                .expect("private key pem error")
                .to_string();
            let public_pem = public_key
                .to_public_key_pem(LineEnding::LF)
                .expect("public key pem error");

            std::env::set_var("JWT_PUBLIC_KEY", &public_pem);

            if let Ok(dec_key) = jsonwebtoken::DecodingKey::from_rsa_pem(public_pem.as_bytes()) {
                set_custom_decoding_key(dec_key);
            }

            TestEnv {
                private_pem,
                public_pem,
            }
        })
    }

    fn make_token_sub(sub: &str, tenant_id: Uuid, profile: &str) -> String {
        let env = get_test_env();
        let encoding_key = EncodingKey::from_rsa_pem(env.private_pem.as_bytes()).unwrap();
        let now = Utc::now().timestamp() as usize;

        let claims = Claims {
            sub: sub.to_string(),
            tenant_id: Some(tenant_id.to_string()),
            email: Some("unit_test@hrm.com".to_string()),
            name: Some("Unit Test User".to_string()),
            profile: Some(profile.to_string()),
            iss: Some("unified_login".to_string()),
            aud: Some(json!(["hrm-api"])),
            exp: Some(now + 3600),
        };

        let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some("unit-test-key".to_string());
        encode(&header, &claims, &encoding_key).unwrap()
    }

    fn make_token(user_id: Uuid, tenant_id: Uuid, profile: &str) -> String {
        make_token_sub(&user_id.to_string(), tenant_id, profile)
    }

    async fn get_pool_and_setup() -> sqlx::PgPool {
        let _ = get_test_env();
        let pool = create_pool().await.expect("Database connection failed in tests");


        let ddl_statements = [
            r#"CREATE TABLE IF NOT EXISTS categories (
                id UUID PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                tenant_id UUID,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                CONSTRAINT uq_unit_cat UNIQUE (tenant_id, name)
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
            r#"CREATE TABLE IF NOT EXISTS activity_contacts (
                id UUID PRIMARY KEY,
                activity_id UUID NOT NULL,
                contact_id UUID NOT NULL,
                tenant_id UUID NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                CONSTRAINT uq_unit_act_cnt UNIQUE (activity_id, contact_id)
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
            "ALTER TABLE activity_contacts ALTER COLUMN created_at TYPE TIMESTAMPTZ, ALTER COLUMN updated_at TYPE TIMESTAMPTZ;",
        ];

        for stmt in alter_statements {
            let _ = sqlx::query(stmt).execute(&pool).await;
        }

        pool
    }



    #[tokio::test]
    async fn test_full_app_routes_and_handlers() {
        let pool = get_pool_and_setup().await;

        let app = create_app(pool.clone());

        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let token = make_token(user_id, tenant_id, "admin");

        // Health Root & Up
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder().uri("/up").body(Body::empty()).unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Dashboard
        let req = Request::builder()
            .uri("/dashboard/stats")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8_lossy(&bytes);
        assert_eq!(status, StatusCode::OK, "Dashboard stats failed: {}", body_str);


        // Categories CRUD
        let cat_payload = json!({ "category": { "name": "Vendas Unit" } });
        let req = Request::builder()
            .method("POST")
            .uri("/categories")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(cat_payload.to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let cat: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(json!({ "error": String::from_utf8_lossy(&bytes) }));
        assert_eq!(status, StatusCode::CREATED, "Create category failed: {:?}", cat);
        let cat_id = cat["id"].as_str().unwrap();

        let req = Request::builder()
            .method("GET")
            .uri("/categories")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .method("GET")
            .uri(format!("/categories/{}", cat_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .method("PUT")
            .uri(format!("/categories/{}", cat_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "category": { "name": "Vendas Unit Updated" } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Companies CRUD
        let comp_payload = json!({
            "company": {
                "name": "Unit Comp Corp",
                "cnpj": "99.888.777/0001-66",
                "website": "https://unitcomp.com",
                "categories": ["Vendas Unit Updated"]
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/companies")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(comp_payload.to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let comp: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let comp_id = comp["id"].as_str().unwrap();

        let req = Request::builder()
            .method("GET")
            .uri(format!("/companies/{}", comp_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .method("GET")
            .uri("/companies?name=Unit&category=Vendas+Unit+Updated")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .method("PUT")
            .uri(format!("/companies/{}", comp_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "company": { "name": "Unit Comp Corp 2" } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Contacts CRUD
        let contact_payload = json!({
            "contact": {
                "company_id": comp_id,
                "name": "Joao Unit",
                "email": "joao@unitcomp.com",
                "phone": "+55 11 9999-1111",
                "job_title": "Manager"
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/contacts")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(contact_payload.to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let cont: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let cont_id = cont["id"].as_str().unwrap();

        let req = Request::builder()
            .method("GET")
            .uri(format!("/contacts/{}", cont_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .method("GET")
            .uri(format!("/contacts?company_id={}&name=Joao", comp_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .method("PUT")
            .uri(format!("/contacts/{}", cont_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "contact": { "job_title": "Senior Manager" } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Projects CRUD
        let proj_payload = json!({
            "project": {
                "name": "Unit Project",
                "description": "Testing project endpoints",
                "responsible_id": user_id,
                "status": "in_progress",
                "start_date": "2026-09-01",
                "end_date": "2026-12-31"
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/projects")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(proj_payload.to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let proj: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let proj_id = proj["id"].as_str().unwrap();

        let req = Request::builder()
            .method("GET")
            .uri(format!("/projects/{}", proj_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .method("GET")
            .uri("/projects?status=in_progress")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .method("PUT")
            .uri(format!("/projects/{}", proj_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "project": { "name": "Unit Project Updated", "status": "in_progress" } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Activities CRUD
        let act_payload = json!({
            "activity": {
                "title": "Unit Activity Test",
                "responsible_id": user_id,
                "status": "pending",
                "due_date": "2026-10-15T10:00:00Z"
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri(format!("/projects/{}/activities", proj_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(act_payload.to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let act: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let act_id = act["id"].as_str().unwrap();

        let req = Request::builder()
            .method("PATCH")
            .uri(format!("/activities/{}", act_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "activity": { "status": "completed" } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Leads CRUD & Conversion
        let lead_payload = json!({
            "lead": {
                "name": "Maria Lead Unit",
                "company_name": "Maria Agency",
                "email": "maria@agency.com",
                "phone": "+55 11 9876-5432",
                "job_title": "Director",
                "categories": ["Vendas Unit Updated"]
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/leads")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(lead_payload.to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let lead: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let lead_id = lead["id"].as_str().unwrap();

        let req = Request::builder()
            .method("GET")
            .uri(format!("/leads/{}", lead_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .method("GET")
            .uri("/leads?status=pending")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .method("PUT")
            .uri(format!("/leads/{}", lead_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "lead": { "notes": "Cold contact" } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .method("POST")
            .uri(format!("/leads/{}/convert", lead_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Expenses CRUD
        let exp_payload = json!({
            "expense": {
                "amount": "88.90",
                "date": "2026-09-27",
                "description": "Taxi para cliente (Taxi 99)",
                "establishment": "Taxi 99",
                "project_id": proj_id
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/expenses")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(exp_payload.to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let exp: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let exp_id = exp["id"].as_str().unwrap();

        let req = Request::builder()
            .method("GET")
            .uri(format!("/expenses/{}", exp_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .method("GET")
            .uri("/expenses?month=9&year=2026")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .method("PUT")
            .uri(format!("/expenses/{}", exp_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "expense": { "amount": "95.00" } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Leads Delete
        let req = Request::builder()
            .method("DELETE")
            .uri(format!("/leads/{}", lead_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        // Smart Input Commit endpoint
        let commit_payload = json!({
            "payload": {
                "companies": [{ "name": "Smart Company", "categories": ["Tecnologia"] }],
                "contacts": [{ "name": "Smart Contact", "company_name": "Smart Company" }],
                "projects": [{ "name": "Smart Project" }],
                "activities": [{ "title": "Smart Meeting", "project_name": "Smart Project" }],
                "expenses": [{ "amount": 42.0, "description": "Smart Expense", "project_name": "Smart Project" }]
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/smart_input/commit")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(commit_payload.to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_users_proxy_handler() {
        let _ = get_test_env();
        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let token = make_token(user_id, tenant_id, "admin");

        // 1. Setup mock server returning OK (200)
        let mock_app = axum::Router::new()
            .route("/users.json", axum::routing::get(|| async {
                (StatusCode::OK, axum::Json(json!([
                    { "id": "1", "name": "Admin User", "email": "admin@hrm.com" }
                ])))
            }));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, mock_app).await.unwrap();
        });

        let mut cfg = AppConfig::get();
        cfg.unified_login_url = format!("http://{}", addr);
        AppConfig::set_custom_config(cfg.clone());

        let pool = get_pool_and_setup().await;
        let app = create_app(pool.clone());

        // 2. Test successful proxy request
        let req = Request::builder()
            .method("GET")
            .uri("/users")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body[0]["name"], "Admin User");

        // 3. Test 403 Forbidden from Identity Provider
        let mock_err_app = axum::Router::new()
            .route("/users.json", axum::routing::get(|| async {
                (StatusCode::FORBIDDEN, axum::Json(json!({ "error": "Access denied" })))
            }));
        let err_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let err_addr = err_listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(err_listener, mock_err_app).await.unwrap();
        });

        cfg.unified_login_url = format!("http://{}", err_addr);
        AppConfig::set_custom_config(cfg.clone());

        let req = Request::builder()
            .method("GET")
            .uri("/users")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);

        // 4. Test Bad Gateway when server is unreachable
        cfg.unified_login_url = "http://127.0.0.1:59999".to_string();
        AppConfig::set_custom_config(cfg);

        let req = Request::builder()
            .method("GET")
            .uri("/users")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn test_leads_file_processor_csv_parsing() {
        let pool = get_pool_and_setup().await;
        let tenant_id = Uuid::new_v4();
        let file_id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO leads_files (id, tenant_id, created_at, updated_at) VALUES ($1, $2, NOW(), NOW())"
        )
        .bind(file_id)
        .bind(tenant_id)
        .execute(&pool)
        .await
        .unwrap();

        // CSV with Semicolon, BOM, various headers, multiple categories
        let csv_data = "\u{feff}Nome;Empresa;Email;Telefone;Cargo;Site;Cidade;Estado;Observacoes;Categorias\n\
Ana Lima;Lima SA;ana@lima.com;11999991111;Diretora;https://lima.com;Sao Paulo;SP;Cliente potencial;SaaS, Consultoria\n\
Bruno Dias;Dias ME;bruno@dias.com;21988882222;Gerente;;Rio de Janeiro;RJ;Feira de negocios;Tecnologia\n\
;;;;;;;;;\n\
Carlos Sem Empresa;;carlos@freelance.com;;;;;;;\n";

        let res = LeadsFileProcessor::do_process_file(&pool, file_id, tenant_id, csv_data.as_bytes()).await;
        assert!(res.is_ok());

        // Check leads were created
        let leads: Vec<Lead> = sqlx::query_as("SELECT * FROM leads WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(leads.len(), 3);

        // Check categories were registered
        let cats: Vec<Category> = sqlx::query_as("SELECT * FROM categories WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_all(&pool)
            .await
            .unwrap();
        assert!(cats.len() >= 2);

        // Check leads_file_lines
        let lines: Vec<LeadsFileLine> = sqlx::query_as("SELECT * FROM leads_file_lines WHERE leads_file_id = $1")
            .bind(file_id)
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|l| l.status == 3));
    }

    #[tokio::test]
    async fn test_storage_service_unit() {
        let pool = get_pool_and_setup().await;
        let record_id = Uuid::new_v4();
        let filename = "contract_test.pdf";
        let content_type = "application/pdf";
        let data = b"%PDF-1.4 test binary content data";

        let blob_id = StorageService::save_attachment(
            &pool,
            "Contract",
            record_id,
            "document",
            filename,
            content_type,
            data,
        )
        .await
        .unwrap();

        assert!(!blob_id.is_nil());

        let loaded = StorageService::get_attachment(&pool, "Contract", record_id, "document")
            .await
            .unwrap();
        assert!(loaded.is_some());
        let (blob, bytes) = loaded.unwrap();
        assert_eq!(blob.filename, filename);
        assert_eq!(blob.content_type.as_deref(), Some(content_type));
        assert_eq!(bytes, data);

        let none_loaded = StorageService::get_attachment(&pool, "Contract", Uuid::new_v4(), "document")
            .await
            .unwrap();
        assert!(none_loaded.is_none());
    }

    #[tokio::test]
    async fn test_leads_files_multipart_and_download() {
        let pool = get_pool_and_setup().await;
        let app = create_app(pool.clone());
        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let token = make_token(user_id, tenant_id, "admin");

        // 1. Test POST without multipart file -> 422
        let req = Request::builder()
            .method("POST")
            .uri("/leads_files")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "multipart/form-data; boundary=boundary123")
            .body(Body::from("--boundary123--\r\n"))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

        // 2. Test POST with valid multipart file
        let boundary = "---------------------------974767299852498929531610575";
        let body_bytes = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"my_leads.csv\"\r\n\
             Content-Type: text/csv\r\n\r\n\
             name,email\nLead Alpha,alpha@test.com\n\
             \r\n--{boundary}--\r\n",
            boundary = boundary
        );

        let req = Request::builder()
            .method("POST")
            .uri("/leads_files")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={}", boundary))
            .body(Body::from(body_bytes))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::ACCEPTED);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let res_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let file_id = res_json["id"].as_str().unwrap();

        // 3. Test Download leads file -> 200
        let req = Request::builder()
            .method("GET")
            .uri(format!("/leads_files/{}/download", file_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers().get(header::CONTENT_TYPE).unwrap(), "text/csv");

        // 4. Test Download nonexistent -> 404
        let req = Request::builder()
            .method("GET")
            .uri(format!("/leads_files/{}/download", Uuid::new_v4()))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_auth_edge_cases_and_roles() {
        let pool = get_pool_and_setup().await;
        let app = create_app(pool.clone());
        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();

        // 1. Missing auth header -> 401
        let req = Request::builder().uri("/companies").body(Body::empty()).unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // 2. Non-bearer header -> 401
        let req = Request::builder()
            .uri("/companies")
            .header(header::AUTHORIZATION, "Basic user:pass")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // 3. Super admin vs regular admin role checks
        let super_admin_token = make_token(user_id, tenant_id, "super_admin");
        let client_token = make_token_sub("app_client_1", tenant_id, "app_client");
        let user_token = make_token(user_id, tenant_id, "user");

        // Super admin creates global category
        let unique_global_name = format!("Global Shared Category {}", Uuid::new_v4());
        let global_cat_payload = json!({
            "category": {
                "name": unique_global_name,
                "global": true
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/categories")
            .header(header::AUTHORIZATION, format!("Bearer {}", super_admin_token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(global_cat_payload.to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let cat: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let global_cat_id = cat["id"].as_str().unwrap();
        assert!(cat["tenant_id"].is_null());

        // Regular user cannot create category -> 403
        let req = Request::builder()
            .method("POST")
            .uri("/categories")
            .header(header::AUTHORIZATION, format!("Bearer {}", user_token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "category": { "name": format!("User Cat {}", Uuid::new_v4()) } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);

        // Regular admin cannot update global category -> 403
        let admin_token = make_token(user_id, tenant_id, "admin");
        let req = Request::builder()
            .method("PUT")
            .uri(format!("/categories/{}", global_cat_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "category": { "name": "Attempt Rename" } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);

        // App client can create category
        let req = Request::builder()
            .method("POST")
            .uri("/categories")
            .header(header::AUTHORIZATION, format!("Bearer {}", client_token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "category": { "name": format!("Client Cat {}", Uuid::new_v4()) } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_expenses_receipt_and_filter_flows() {
        let pool = get_pool_and_setup().await;
        let app = create_app(pool.clone());
        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let token = make_token(user_id, tenant_id, "admin");

        // 1. Create Expense with base64 receipt
        let b64_data = "data:image/jpeg;base64,/9j/4AAQSkZJRg==";
        let exp_payload = json!({
            "expense": {
                "amount": "145.20",
                "date": "2026-09-15",
                "description": "Estacionamento Aeroporto"
            },
            "receipt": {
                "data": b64_data,
                "filename": "ticket_rec.jpg",
                "content_type": "image/jpeg"
            }
        });

        let req = Request::builder()
            .method("POST")
            .uri("/expenses")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(exp_payload.to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let exp: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let exp_id = exp["id"].as_str().unwrap();

        // 2. Get Receipt with inline disposition
        let req = Request::builder()
            .method("GET")
            .uri(format!("/expenses/{}/receipt?disposition=inline", exp_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(res.headers().get(header::CONTENT_DISPOSITION).unwrap().to_str().unwrap().contains("inline"));

        // 3. Get Receipt for nonexistent expense -> 404
        let req = Request::builder()
            .method("GET")
            .uri(format!("/expenses/{}/receipt", Uuid::new_v4()))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        // 4. Filter expenses by date range and project_id = "none"
        let req = Request::builder()
            .method("GET")
            .uri("/expenses?start_date=2026-09-01&end_date=2026-09-30&project_id=none")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(list["expenses"].as_array().unwrap().len() >= 1);

        // 5. Validation error: negative amount -> 422
        let invalid_exp = json!({
            "expense": {
                "amount": "-50.00",
                "date": "2026-09-15",
                "description": "Invalid Amount"
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/expenses")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(invalid_exp.to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_smart_input_analyze_and_builder_coverage() {
        let pool = get_pool_and_setup().await;
        let app = create_app(pool.clone());
        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let token = make_token(user_id, tenant_id, "admin");

        // 1. Analyze endpoint with empty body -> 422
        let req = Request::builder()
            .method("POST")
            .uri("/smart_input/analyze")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({}).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

        // 2. Analyze endpoint with text (no API key configured) -> 500
        let req = Request::builder()
            .method("POST")
            .uri("/smart_input/analyze")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "text": "Almoco no fasano 150 reais", "provider": "deepseek" }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // 3. Analyze endpoint with image object and string formats
        let req = Request::builder()
            .method("POST")
            .uri("/smart_input/analyze")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "image": { "content_type": "image/png", "data": "data:image/png;base64,iVBORw0KGgo=" },
                "provider": "gemini"
            }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // 4. Builder commit with merging company categories, existing project, default project, establishment in expense
        let auth_user = AuthUser {
            user_id: user_id.to_string(),
            tenant_id,
            email: Some("admin@hrm.com".to_string()),
            name: Some("Admin".to_string()),
            profile: Some("admin".to_string()),
            raw_token: token.clone(),
        };

        let unique_comp = format!("Builder Comp {}", Uuid::new_v4());
        let unique_proj = format!("Builder Proj {}", Uuid::new_v4());

        // First commit
        let payload1 = json!({
            "companies": [{ "name": unique_comp, "categories": ["Cat A"] }],
            "projects": [{ "name": unique_proj, "status": "in_progress" }]
        });
        let msg1 = EntityBuilderService::commit(&pool, &payload1, &auth_user).await.unwrap();
        assert!(msg1.contains("sucesso"));

        // Second commit: merges category into existing company, attaches contact, adds activity to existing proj and default proj, adds expense with establishment
        let b64_receipt = "data:image/jpeg;base64,/9j/4AAQSkZJRg==";
        let payload2 = json!({
            "companies": [{ "name": unique_comp, "categories": ["Cat B"] }],
            "contacts": [
                { "name": "Contact 1", "company_name": unique_comp, "email": "c1@test.com" },
                { "name": "Contact 2", "company_name": format!("Nonexistent Comp {}", Uuid::new_v4()) }
            ],
            "activities": [
                { "title": "Meeting A", "project_name": unique_proj, "due_date": "2026-11-01" },
                { "title": "Meeting B", "due_date": "2026-11-02" } // default project
            ],
            "expenses": [
                { "amount": "99.50", "description": "Almoco", "establishment": "Fasano", "project_name": unique_proj, "receipt_image": b64_receipt },
                { "amount": 0.0, "description": "Ignored Expense" }
            ]
        });
        let msg2 = EntityBuilderService::commit(&pool, &payload2, &auth_user).await.unwrap();
        assert!(msg2.contains("sucesso"));
    }

    #[tokio::test]
    async fn test_activities_error_cases_and_contacts() {
        let pool = get_pool_and_setup().await;
        let app = create_app(pool.clone());
        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let token = make_token(user_id, tenant_id, "admin");

        // 1. Create activity for nonexistent project -> 404
        let req = Request::builder()
            .method("POST")
            .uri(format!("/projects/{}/activities", Uuid::new_v4()))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "activity": { "title": "Act", "responsible_id": user_id }
            }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        // 2. Create project and contact first
        let proj_id = Uuid::new_v4();
        let comp_id = Uuid::new_v4();
        let cont_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query("INSERT INTO projects (id, name, responsible_id, tenant_id, created_at, updated_at) VALUES ($1, 'Act Proj', $2, $3, $4, $4)")
            .bind(proj_id).bind(user_id).bind(tenant_id).bind(now).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO companies (id, name, tenant_id, created_at, updated_at) VALUES ($1, 'Act Comp', $2, $3, $3)")
            .bind(comp_id).bind(tenant_id).bind(now).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO contacts (id, company_id, name, email, tenant_id, created_at, updated_at) VALUES ($1, $2, 'Act Cont', 'c@act.com', $3, $4, $4)")
            .bind(cont_id).bind(comp_id).bind(tenant_id).bind(now).execute(&pool).await.unwrap();

        // 3. Create activity with invalid status -> 422
        let req = Request::builder()
            .method("POST")
            .uri(format!("/projects/{}/activities", proj_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "activity": { "title": "Act", "responsible_id": user_id, "status": "invalid_status" }
            }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

        // 4. Create activity with contact_ids -> 201
        let req = Request::builder()
            .method("POST")
            .uri(format!("/projects/{}/activities", proj_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "activity": {
                    "title": "Act With Contacts",
                    "responsible_id": user_id,
                    "contact_ids": [cont_id]
                }
            }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let act: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let act_id = act["id"].as_str().unwrap();
        assert_eq!(act["contacts"].as_array().unwrap().len(), 1);

        // 5. Update activity with blank title -> 422
        let req = Request::builder()
            .method("PUT")
            .uri(format!("/activities/{}", act_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "activity": { "title": "  " } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

        // 6. Update nonexistent activity -> 404
        let req = Request::builder()
            .method("PUT")
            .uri(format!("/activities/{}", Uuid::new_v4()))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "activity": { "title": "New" } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_companies_and_projects_filters_and_errors() {
        let pool = get_pool_and_setup().await;
        let app = create_app(pool.clone());
        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let token = make_token(user_id, tenant_id, "admin");

        // 1. Company with non-registered category -> 422
        let req = Request::builder()
            .method("POST")
            .uri("/companies")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "company": { "name": "Fake Comp", "categories": ["Nonexistent Category 999"] }
            }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

        // 2. Update nonexistent company -> 404
        let req = Request::builder()
            .method("PUT")
            .uri(format!("/companies/{}", Uuid::new_v4()))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "company": { "name": "Updated" } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        // 3. Project with details (activities + expenses)
        let proj_id = Uuid::new_v4();
        let now = Utc::now();
        sqlx::query("INSERT INTO projects (id, name, responsible_id, status, tenant_id, created_at, updated_at) VALUES ($1, 'Detail Proj', $2, 'in_progress', $3, $4, $4)")
            .bind(proj_id).bind(user_id).bind(tenant_id).bind(now).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO activities (id, project_id, responsible_id, title, status, tenant_id, created_at, updated_at) VALUES ($1, $2, $3, 'Act Detail', 'pending', $4, $5, $5)")
            .bind(Uuid::new_v4()).bind(proj_id).bind(user_id).bind(tenant_id).bind(now).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO expenses (id, amount, date, description, project_id, responsible_id, tenant_id, created_at, updated_at) VALUES ($1, 55.0, CURRENT_DATE, 'Exp Detail', $2, $3, $4, $5, $5)")
            .bind(Uuid::new_v4()).bind(proj_id).bind(user_id).bind(tenant_id).bind(now).execute(&pool).await.unwrap();

        let req = Request::builder()
            .method("GET")
            .uri(format!("/projects/{}", proj_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let proj_details: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(proj_details["activities"].as_array().unwrap().len(), 1);
        assert_eq!(proj_details["expenses"].as_array().unwrap().len(), 1);

        // 4. Update nonexistent project -> 404
        let req = Request::builder()
            .method("PUT")
            .uri(format!("/projects/{}", Uuid::new_v4()))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "project": { "name": "New Name" } }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_leads_filters_and_conversion_existing_company() {
        let pool = get_pool_and_setup().await;
        let app = create_app(pool.clone());
        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let token = make_token(user_id, tenant_id, "admin");

        // 1. Create company first
        let existing_comp_name = format!("Corp {}", Uuid::new_v4());
        let comp_id = Uuid::new_v4();
        let now = Utc::now();
        sqlx::query("INSERT INTO companies (id, name, tenant_id, created_at, updated_at) VALUES ($1, $2, $3, $4, $4)")
            .bind(comp_id).bind(&existing_comp_name).bind(tenant_id).bind(now).execute(&pool).await.unwrap();

        // 2. Create lead referencing that company
        let lead_id = Uuid::new_v4();
        sqlx::query("INSERT INTO leads (id, name, company_name, email, status, tenant_id, created_at, updated_at) VALUES ($1, 'Lead For Corp', $2, 'lead@corp.com', 'new', $3, $4, $4)")
            .bind(lead_id).bind(&existing_comp_name).bind(tenant_id).bind(now).execute(&pool).await.unwrap();

        // 3. Convert lead -> reuses existing company
        let req = Request::builder()
            .method("POST")
            .uri(format!("/leads/{}/convert", lead_id))
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let conversion: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(conversion["company"]["id"], comp_id.to_string());

        // 4. Nonexistent lead operations -> 404
        let nonexistent = Uuid::new_v4();
        let req = Request::builder().method("GET").uri(format!("/leads/{}", nonexistent)).header(header::AUTHORIZATION, format!("Bearer {}", token)).body(Body::empty()).unwrap();
        assert_eq!(app.clone().oneshot(req).await.unwrap().status(), StatusCode::NOT_FOUND);

        let req = Request::builder().method("PUT").uri(format!("/leads/{}", nonexistent)).header(header::AUTHORIZATION, format!("Bearer {}", token)).header(header::CONTENT_TYPE, "application/json").body(Body::from(json!({ "lead": { "name": "New" } }).to_string())).unwrap();
        assert_eq!(app.clone().oneshot(req).await.unwrap().status(), StatusCode::NOT_FOUND);

        let req = Request::builder().method("DELETE").uri(format!("/leads/{}", nonexistent)).header(header::AUTHORIZATION, format!("Bearer {}", token)).body(Body::empty()).unwrap();
        assert_eq!(app.clone().oneshot(req).await.unwrap().status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_auth_jwt_validation_matrix() {
        let _ = get_test_env();
        let env = get_test_env();
        let encoding_key = EncodingKey::from_rsa_pem(env.private_pem.as_bytes()).unwrap();
        let now = Utc::now().timestamp() as usize;

        // 1. Invalid audience
        let claims = Claims {
            sub: Uuid::new_v4().to_string(),
            tenant_id: Some(Uuid::new_v4().to_string()),
            email: Some("test@test.com".to_string()),
            name: Some("Test".to_string()),
            profile: Some("admin".to_string()),
            iss: Some("unified_login".to_string()),
            aud: Some(json!(["invalid-audience"])),
            exp: Some(now + 3600),
        };
        let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
        let token = encode(&header, &claims, &encoding_key).unwrap();
        assert!(verify_jwt(&token).is_err());

        // 2. Single string audience: "crm-api"
        let mut claims_crm = claims;
        claims_crm.aud = Some(json!("crm-api"));
        let token_crm = encode(&header, &claims_crm, &encoding_key).unwrap();
        let verified = verify_jwt(&token_crm).unwrap();
        assert_eq!(verified.profile.as_deref(), Some("admin"));
        assert!(verified.is_admin());
        assert!(!verified.is_super_admin());
        assert!(!verified.is_app_client());
        assert!(verified.user_uuid().is_some());

        // 3. Super user profile
        let mut claims_super = claims_crm.clone();
        claims_super.profile = Some("super_user".to_string());
        claims_super.sub = "app_client_99".to_string();
        let token_super = encode(&header, &claims_super, &encoding_key).unwrap();
        let verified_super = verify_jwt(&token_super).unwrap();
        assert!(verified_super.is_super_admin());
        assert!(verified_super.is_admin());
        assert!(verified_super.is_app_client());
        assert!(verified_super.user_uuid().is_none());

        // 4. Missing tenant context
        let mut claims_no_tenant = claims_super;
        claims_no_tenant.tenant_id = None;
        let token_no_tenant = encode(&header, &claims_no_tenant, &encoding_key).unwrap();
        assert!(verify_jwt(&token_no_tenant).is_err());

        // 5. Invalid tenant UUID
        let mut claims_bad_tenant = claims_no_tenant;
        claims_bad_tenant.tenant_id = Some("not-a-uuid".to_string());
        let token_bad_tenant = encode(&header, &claims_bad_tenant, &encoding_key).unwrap();
        assert!(verify_jwt(&token_bad_tenant).is_err());

        // 6. Algorithm mismatch (HS256)
        header.alg = jsonwebtoken::Algorithm::HS256;
        let token_hs = encode(&header, &claims_crm, &EncodingKey::from_secret(b"secret")).unwrap();
        assert!(verify_jwt(&token_hs).is_err());
    }

    #[tokio::test]
    async fn test_companies_filter() {
        let pool = get_pool_and_setup().await;
        let app = create_app(pool.clone());
        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let token = make_token(user_id, tenant_id, "admin");

        // Filter companies by category
        let req = Request::builder()
            .method("GET")
            .uri("/companies?category=Tecnologia")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

