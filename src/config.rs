use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub port: u16,
    pub host: String,
    pub unified_login_url: String,
    pub gemini_api_key: Option<String>,
    pub deepseek_api_key: Option<String>,
    pub public_key_path: String,
    pub storage_dir: String,
}

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

impl AppConfig {
    pub fn load() -> &'static AppConfig {
        CONFIG.get_or_init(|| {
            let database_url = std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://development:development@localhost:5432/hrm_development".to_string());
            
            let port = std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000);

            let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

            let unified_login_url = std::env::var("UNIFIED_LOGIN_URL")
                .unwrap_or_else(|_| "http://localhost:3001".to_string());

            let gemini_api_key = std::env::var("GEMINI_API_KEY").ok().filter(|s| !s.is_empty());
            let deepseek_api_key = std::env::var("DEEPSEEK_API_KEY").ok().filter(|s| !s.is_empty());

            let public_key_path = std::env::var("PUBLIC_KEY_PATH")
                .unwrap_or_else(|_| "config/keys/public.pem".to_string());

            let storage_dir = std::env::var("STORAGE_DIR")
                .unwrap_or_else(|_| "storage".to_string());

            AppConfig {
                database_url,
                port,
                host,
                unified_login_url,
                gemini_api_key,
                deepseek_api_key,
                public_key_path,
                storage_dir,
            }
        })
    }

    pub fn get() -> &'static AppConfig {
        Self::load()
    }
}
