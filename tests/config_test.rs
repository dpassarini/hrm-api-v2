use hrm_api_v2::config::AppConfig;

#[test]
fn test_config_defaults_and_get() {
    let config = AppConfig::get();
    assert!(!config.database_url.is_empty());
    assert!(config.port > 0);
    assert!(!config.host.is_empty());
    assert!(!config.storage_dir.is_empty());
    assert!(!config.unified_login_url.is_empty());
}
