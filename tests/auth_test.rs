mod common;

use hrm_api_v2::auth::verify_jwt;
use uuid::Uuid;

#[test]
fn test_verify_valid_jwt() {
    let _ = common::get_test_keys();
    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();

    let token = common::create_valid_token(user_id, tenant_id, "admin");
    let auth_user = verify_jwt(&token).expect("Token verification failed");

    assert_eq!(auth_user.user_id, user_id.to_string());
    assert_eq!(auth_user.tenant_id, tenant_id);
    assert_eq!(auth_user.profile.as_deref(), Some("admin"));
    assert!(auth_user.is_admin());
    assert!(!auth_user.is_super_admin());
    assert!(!auth_user.is_app_client());
    assert_eq!(auth_user.user_uuid(), Some(user_id));
}

#[test]
fn test_super_admin_roles() {
    let _ = common::get_test_keys();
    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();

    let token1 = common::create_valid_token(user_id, tenant_id, "super_admin");
    let user1 = verify_jwt(&token1).unwrap();
    assert!(user1.is_super_admin());
    assert!(user1.is_admin());

    let token2 = common::create_valid_token(user_id, tenant_id, "super_user");
    let user2 = verify_jwt(&token2).unwrap();
    assert!(user2.is_super_admin());
    assert!(user2.is_admin());
}

#[test]
fn test_app_client_role() {
    let _ = common::get_test_keys();
    let tenant_id = Uuid::new_v4();

    let token = common::create_test_token(
        "app_service_123",
        Some(&tenant_id.to_string()),
        Some("client"),
        None,
        None,
        false,
        None,
        None,
    );

    let user = verify_jwt(&token).unwrap();
    assert!(user.is_app_client());
    assert!(!user.is_admin());
    assert_eq!(user.user_uuid(), None);
}

#[test]
fn test_expired_jwt() {
    let _ = common::get_test_keys();
    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();

    let token = common::create_test_token(
        &user_id.to_string(),
        Some(&tenant_id.to_string()),
        Some("user"),
        None,
        None,
        true, // expired
        None,
        None,
    );

    let res = verify_jwt(&token);
    assert!(res.is_err());
}

#[test]
fn test_invalid_issuer() {
    let _ = common::get_test_keys();
    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();

    let token = common::create_test_token(
        &user_id.to_string(),
        Some(&tenant_id.to_string()),
        Some("user"),
        None,
        None,
        false,
        Some("wrong_issuer"),
        None,
    );

    let res = verify_jwt(&token);
    assert!(res.is_err());
}

#[test]
fn test_invalid_audience() {
    let _ = common::get_test_keys();
    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();

    let token = common::create_test_token(
        &user_id.to_string(),
        Some(&tenant_id.to_string()),
        Some("user"),
        None,
        None,
        false,
        None,
        Some(serde_json::json!(["other-api"])),
    );

    let res = verify_jwt(&token);
    assert!(res.is_err());
}

#[test]
fn test_missing_or_invalid_tenant_id() {
    let _ = common::get_test_keys();
    let user_id = Uuid::new_v4();

    // None tenant_id
    let token1 = common::create_test_token(
        &user_id.to_string(),
        None,
        Some("user"),
        None,
        None,
        false,
        None,
        None,
    );
    assert!(verify_jwt(&token1).is_err());

    // "missing" tenant_id
    let token2 = common::create_test_token(
        &user_id.to_string(),
        Some("missing"),
        Some("user"),
        None,
        None,
        false,
        None,
        None,
    );
    assert!(verify_jwt(&token2).is_err());

    // Invalid UUID
    let token3 = common::create_test_token(
        &user_id.to_string(),
        Some("not-a-valid-uuid"),
        Some("user"),
        None,
        None,
        false,
        None,
        None,
    );
    assert!(verify_jwt(&token3).is_err());
}
