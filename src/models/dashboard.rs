use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadsCountStats {
    pub total: i64,
    pub new: i64,
    pub contacted: i64,
    pub qualified: i64,
    pub converted: i64,
    pub lost: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectsCountStats {
    pub total: i64,
    pub planned: i64,
    pub in_progress: i64,
    pub completed: i64,
    pub cancelled: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivitiesCountStats {
    pub total: i64,
    pub pending: i64,
    pub completed: i64,
    pub cancelled: i64,
    pub overdue: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseByProject {
    pub project_name: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseByMonth {
    pub month: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalActivity {
    pub id: Uuid,
    pub title: String,
    pub due_date: Option<DateTime<Utc>>,
    pub project_name: String,
    pub overdue: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopCompany {
    pub name: String,
    pub contacts_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_expense_amount: f64,
    pub companies_count: i64,
    pub contacts_count: i64,
    pub leads_count: LeadsCountStats,
    pub projects_count: ProjectsCountStats,
    pub activities_count: ActivitiesCountStats,
    pub expenses_by_project: Vec<ExpenseByProject>,
    pub expenses_by_month: Vec<ExpenseByMonth>,
    pub critical_activities: Vec<CriticalActivity>,
    pub top_companies: Vec<TopCompany>,
}
