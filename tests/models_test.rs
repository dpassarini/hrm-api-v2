use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use hrm_api_v2::models::{
    company::*, dashboard::*, expense::*, pagination::*,
};
use std::str::FromStr;
use uuid::Uuid;

#[test]
fn test_pagination_meta() {
    let meta1 = PaginationMeta::new(25, 10);
    assert_eq!(meta1.total_count, 25);
    assert_eq!(meta1.total_pages, 3);

    let meta2 = PaginationMeta::new(0, 10);
    assert_eq!(meta2.total_count, 0);
    assert_eq!(meta2.total_pages, 0);

    let meta3 = PaginationMeta::new(30, 10);
    assert_eq!(meta3.total_pages, 3);
}

#[test]
fn test_pagination_query_defaults() {
    let q = PaginationQuery {
        page: None,
        per_page: None,
    };
    assert_eq!(q.page(), 1);
    assert_eq!(q.per_page(), 10);
    assert_eq!(q.offset(), 0);

    let q2 = PaginationQuery {
        page: Some(3),
        per_page: Some(25),
    };
    assert_eq!(q2.page(), 3);
    assert_eq!(q2.per_page(), 25);
    assert_eq!(q2.offset(), 50);

    // Negative and excessive bounds
    let q3 = PaginationQuery {
        page: Some(-5),
        per_page: Some(500),
    };
    assert_eq!(q3.page(), 1);
    assert_eq!(q3.per_page(), 100); // capped at 100
}

#[test]
fn test_company_json_serialization() {
    let comp = Company {
        id: Uuid::new_v4(),
        name: Some("Hotel Fasano".to_string()),
        cnpj: Some("12.345.678/0001-90".to_string()),
        website: Some("https://fasano.com.br".to_string()),
        categories: Some(vec!["Hotelaria".to_string(), "Luxo".to_string()]),
        tenant_id: Some(Uuid::new_v4()),
        created_at: Some(chrono::Utc::now()),
        updated_at: Some(chrono::Utc::now()),
    };

    let serialized = serde_json::to_string(&comp).unwrap();
    let deserialized: Company = serde_json::from_str(&serialized).unwrap();
    assert_eq!(comp.name, deserialized.name);
    assert_eq!(comp.categories, deserialized.categories);
}

#[test]
fn test_expense_and_receipt_serialization() {
    let expense = ExpenseWithProject {
        id: Uuid::new_v4(),
        amount: BigDecimal::from_str("189.50").unwrap(),
        date: NaiveDate::from_ymd_opt(2026, 9, 27).unwrap(),
        description: "Almoço comercial".to_string(),
        project_id: Some(Uuid::new_v4()),
        responsible_id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        project: Some(ExpenseProjectSummary {
            id: Uuid::new_v4(),
            name: "Expansão SP".to_string(),
            status: "in_progress".to_string(),
        }),
        receipt: Some(ReceiptInfo {
            attached: true,
            filename: Some("recibo.pdf".to_string()),
            byte_size: Some(1024),
            content_type: Some("application/pdf".to_string()),
            created_at: Some(chrono::Utc::now()),
        }),
    };

    let json_str = serde_json::to_string(&expense).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(val["description"], "Almoço comercial");
    assert_eq!(val["receipt"]["attached"], true);
    assert_eq!(val["receipt"]["filename"], "recibo.pdf");
}

#[test]
fn test_dashboard_stats_serialization() {
    let stats = DashboardStats {
        total_expense_amount: 1540.50,
        companies_count: 12,
        contacts_count: 34,
        leads_count: LeadsCountStats {
            total: 50,
            new: 20,
            contacted: 15,
            qualified: 10,
            converted: 3,
            lost: 2,
        },
        projects_count: ProjectsCountStats {
            total: 5,
            planned: 2,
            in_progress: 2,
            completed: 1,
            cancelled: 0,
        },
        activities_count: ActivitiesCountStats {
            total: 8,
            pending: 4,
            completed: 3,
            cancelled: 1,
            overdue: 1,
        },
        expenses_by_project: vec![ExpenseByProject {
            project_name: "Projeto A".to_string(),
            amount: 500.0,
        }],
        expenses_by_month: vec![ExpenseByMonth {
            month: "Set/26".to_string(),
            amount: 1540.50,
        }],
        critical_activities: vec![],
        top_companies: vec![],
    };

    let json_str = serde_json::to_string(&stats).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(val["total_expense_amount"], 1540.50);
    assert_eq!(val["leads_count"]["total"], 50);
}
