mod common;

use hrm_api_v2::auth::AuthUser;
use hrm_api_v2::services::smart_input::builder::EntityBuilderService;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_entity_builder_commit_full() {
    let pool = match common::get_test_pool().await {
        Some(p) => p,
        None => return,
    };

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let auth_user = AuthUser {
        user_id: user_id.to_string(),
        tenant_id,
        email: Some("test@example.com".to_string()),
        name: Some("Builder Test User".to_string()),
        profile: Some("admin".to_string()),
        raw_token: "test-token".to_string(),
    };


    let payload = json!({
        "companies": [
            {
                "name": "Nova Tech Builder",
                "categories": ["Inovacao", "Geral"]
            }
        ],
        "contacts": [
            {
                "name": "Joao Engenheiro",
                "email": "joao@novatech.com",
                "phone": "+55 11 91234-5678",
                "job_title": "Lead Developer",
                "company_name": "Nova Tech Builder"
            }
        ],
        "projects": [
            {
                "name": "Plataforma Cloud 2026",
                "status": "in_progress"
            }
        ],
        "activities": [
            {
                "title": "Homologar deploy",
                "project_name": "Plataforma Cloud 2026",
                "status": "pending",
                "due_date": "2026-10-30"
            },
            {
                "title": "Tarefa sem projeto",
                "status": "pending"
            }
        ],
        "expenses": [
            {
                "amount": 125.80,
                "date": "2026-09-27",
                "description": "Cafe com cliente",
                "establishment": "Starbucks Paulista",
                "project_name": "Plataforma Cloud 2026"
            }
        ]
    });

    let result = EntityBuilderService::commit(&pool, &payload, &auth_user).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Entidades importadas com sucesso para o HRM");
}
