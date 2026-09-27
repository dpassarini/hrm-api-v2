mod auth;
mod config;
mod db;
mod error;
mod handlers;
mod models;
mod services;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::AppConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    // Initialize tracing / logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hrm_api_v2=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::load();
    tracing::info!("Initializing HRM API v2 (Rust)...");

    // Initialize Database Pool
    let pool = db::create_pool().await?;
    tracing::info!("Database connection pool established successfully.");

    // CORS configuration (matching Rack::Cors allow all)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build Axum Router
    let app = Router::new()
        // Health / Root
        .route("/", get(handlers::health::root))
        .route("/up", get(handlers::health::health))
        // Dashboard
        .route("/dashboard/stats", get(handlers::dashboard::get_dashboard_stats))
        // Companies
        .route(
            "/companies",
            get(handlers::companies::list_companies).post(handlers::companies::create_company),
        )
        .route(
            "/companies/:id",
            get(handlers::companies::get_company)
                .patch(handlers::companies::update_company)
                .put(handlers::companies::update_company),
        )
        // Contacts
        .route(
            "/contacts",
            get(handlers::contacts::list_contacts).post(handlers::contacts::create_contact),
        )
        .route(
            "/contacts/:id",
            get(handlers::contacts::get_contact)
                .patch(handlers::contacts::update_contact)
                .put(handlers::contacts::update_contact),
        )
        // Categories
        .route(
            "/categories",
            get(handlers::categories::list_categories).post(handlers::categories::create_category),
        )
        .route(
            "/categories/:id",
            get(handlers::categories::get_category)
                .patch(handlers::categories::update_category)
                .put(handlers::categories::update_category),
        )
        // Users (Identity Provider Proxy)
        .route("/users", get(handlers::users::list_users))
        // Leads
        .route(
            "/leads",
            get(handlers::leads::list_leads).post(handlers::leads::create_lead),
        )
        .route(
            "/leads/:id",
            get(handlers::leads::get_lead)
                .patch(handlers::leads::update_lead)
                .put(handlers::leads::update_lead)
                .delete(handlers::leads::delete_lead),
        )
        .route("/leads/:id/convert", post(handlers::leads::convert_lead))
        // Leads Files
        .route(
            "/leads_files",
            get(handlers::leads_files::list_leads_files).post(handlers::leads_files::create_leads_file),
        )
        .route("/leads_files/:id", get(handlers::leads_files::get_leads_file))
        .route(
            "/leads_files/:id/download",
            get(handlers::leads_files::download_leads_file),
        )
        // Projects
        .route(
            "/projects",
            get(handlers::projects::list_projects).post(handlers::projects::create_project),
        )
        .route(
            "/projects/:id",
            get(handlers::projects::get_project)
                .patch(handlers::projects::update_project)
                .put(handlers::projects::update_project),
        )
        // Activities
        .route(
            "/projects/:project_id/activities",
            post(handlers::activities::create_project_activity),
        )
        .route(
            "/activities/:id",
            axum::routing::patch(handlers::activities::update_activity)
                .put(handlers::activities::update_activity),
        )
        // Expenses
        .route(
            "/expenses",
            get(handlers::expenses::list_expenses).post(handlers::expenses::create_expense),
        )
        .route(
            "/expenses/:id",
            get(handlers::expenses::get_expense)
                .patch(handlers::expenses::update_expense)
                .put(handlers::expenses::update_expense),
        )
        .route("/expenses/:id/receipt", get(handlers::expenses::get_expense_receipt))
        // Smart Input
        .route("/smart_input/analyze", post(handlers::smart_input::analyze))
        .route("/smart_input/commit", post(handlers::smart_input::commit))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(pool);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    tracing::info!("HRM API v2 listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
