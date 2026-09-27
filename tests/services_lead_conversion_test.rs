mod common;

use hrm_api_v2::services::lead_conversion::LeadConversionService;
use uuid::Uuid;


#[tokio::test]
async fn test_lead_conversion_service_direct() {
    let pool = match common::get_test_pool().await {
        Some(p) => p,
        None => return,
    };

    let tenant_id = Uuid::new_v4();
    let lead_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    // Insert lead
    sqlx::query(
        r#"
        INSERT INTO leads (id, name, company_name, email, phone, job_title, website, status, categories, tenant_id, created_at, updated_at)
        VALUES ($1, 'Renata Ramos', 'Ramos Consultoria', 'renata@ramos.com', '+55 11 97777-6666', 'CEO', 'https://ramos.com', 'pending', $2, $3, $4, $4)
        "#,
    )
    .bind(lead_id)
    .bind(vec!["Consultoria".to_string()])
    .bind(tenant_id)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    let res = LeadConversionService::convert(&pool, lead_id, tenant_id).await;
    assert!(res.is_ok());
    let (comp, cont, lead) = res.unwrap();
    assert_eq!(comp.name.as_deref(), Some("Ramos Consultoria"));
    assert_eq!(cont.name.as_deref(), Some("Renata Ramos"));
    assert_eq!(lead.status, "converted");

    // Second conversion fails
    let res2 = LeadConversionService::convert(&pool, lead_id, tenant_id).await;
    assert!(res2.is_err());
}

