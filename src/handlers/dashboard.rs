use axum::{extract::State, response::IntoResponse, Json};
use bigdecimal::{BigDecimal, ToPrimitive};
use chrono::{DateTime, Datelike, Duration, Utc};
use sqlx::{PgPool, Row};
use std::collections::HashMap;

use crate::{
    auth::AuthUser,
    error::AppError,
    models::dashboard::{
        ActivitiesCountStats, CriticalActivity, DashboardStats, ExpenseByMonth, ExpenseByProject,
        LeadsCountStats, ProjectsCountStats, TopCompany,
    },
};

pub async fn get_dashboard_stats(
    State(pool): State<PgPool>,
    user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let tenant_id = user.tenant_id;
    let now = Utc::now();

    // 1. Total expense amount
    let total_expense_bd: Option<BigDecimal> = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(amount), 0)
        FROM expenses
        WHERE tenant_id = $1
        "#,
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    let total_expense_amount = total_expense_bd.and_then(|bd| bd.to_f64()).unwrap_or(0.0);

    // 2. Companies & Contacts counts
    let companies_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM companies WHERE tenant_id = $1"#,
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    let contacts_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM contacts WHERE tenant_id = $1"#,
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    // 3. Leads count by status
    let lead_status_rows = sqlx::query(
        r#"
        SELECT status, COUNT(*) as cnt
        FROM leads
        WHERE tenant_id = $1
        GROUP BY status
        "#,
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let mut leads_map: HashMap<String, i64> = HashMap::new();
    let mut total_leads = 0i64;
    for r in lead_status_rows {
        let status: String = r.get("status");
        let cnt: i64 = r.get("cnt");
        total_leads += cnt;
        leads_map.insert(status, cnt);
    }

    let leads_count = LeadsCountStats {
        total: total_leads,
        new: *leads_map.get("new").unwrap_or(&0),
        contacted: *leads_map.get("contacted").unwrap_or(&0),
        qualified: *leads_map.get("qualified").unwrap_or(&0),
        converted: *leads_map.get("converted").unwrap_or(&0),
        lost: *leads_map.get("lost").unwrap_or(&0),
    };

    // 4. Projects count by status
    let proj_status_rows = sqlx::query(
        r#"
        SELECT status, COUNT(*) as cnt
        FROM projects
        WHERE tenant_id = $1
        GROUP BY status
        "#,
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let mut proj_map: HashMap<String, i64> = HashMap::new();
    let mut total_projects = 0i64;
    for r in proj_status_rows {
        let status: String = r.get("status");
        let cnt: i64 = r.get("cnt");
        total_projects += cnt;
        proj_map.insert(status, cnt);
    }

    let projects_count = ProjectsCountStats {
        total: total_projects,
        planned: *proj_map.get("planned").unwrap_or(&0),
        in_progress: *proj_map.get("in_progress").unwrap_or(&0),
        completed: *proj_map.get("completed").unwrap_or(&0),
        cancelled: *proj_map.get("cancelled").unwrap_or(&0),
    };

    // 5. Activities count by status & overdue
    let act_status_rows = sqlx::query(
        r#"
        SELECT status, COUNT(*) as cnt
        FROM activities
        WHERE tenant_id = $1
        GROUP BY status
        "#,
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let mut act_map: HashMap<String, i64> = HashMap::new();
    let mut total_activities = 0i64;
    for r in act_status_rows {
        let status: String = r.get("status");
        let cnt: i64 = r.get("cnt");
        total_activities += cnt;
        act_map.insert(status, cnt);
    }

    let overdue_activities_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM activities
        WHERE tenant_id = $1 AND status = 'pending' AND due_date < $2
        "#,
    )
    .bind(tenant_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .map_err(AppError::from)?;

    let activities_count = ActivitiesCountStats {
        total: total_activities,
        pending: *act_map.get("pending").unwrap_or(&0),
        completed: *act_map.get("completed").unwrap_or(&0),
        cancelled: *act_map.get("cancelled").unwrap_or(&0),
        overdue: overdue_activities_count,
    };

    // 6. Expenses by Project (Top 5)
    let expenses_by_proj_rows = sqlx::query(
        r#"
        SELECT p.name as project_name, SUM(e.amount) as total_amt
        FROM expenses e
        JOIN projects p ON p.id = e.project_id
        WHERE e.tenant_id = $1
        GROUP BY p.name
        ORDER BY total_amt DESC
        LIMIT 5
        "#,
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let expenses_by_project = expenses_by_proj_rows
        .into_iter()
        .map(|r| {
            let total_amt: Option<BigDecimal> = r.get("total_amt");
            ExpenseByProject {
                project_name: r.get("project_name"),
                amount: total_amt.and_then(|bd| bd.to_f64()).unwrap_or(0.0),
            }
        })
        .collect();

    // 7. Expenses by Month (last 6 months)
    let six_months_ago = (now - Duration::days(180)).date_naive();
    let expenses_by_month_rows = sqlx::query(
        r#"
        SELECT TO_CHAR(date, 'YYYY-MM') as ym, SUM(amount) as amt
        FROM expenses
        WHERE tenant_id = $1 AND date >= $2
        GROUP BY ym
        "#,
    )
    .bind(tenant_id)
    .bind(six_months_ago)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let mut monthly_map: HashMap<String, f64> = HashMap::new();
    for r in expenses_by_month_rows {
        let ym: Option<String> = r.get("ym");
        let amt: Option<BigDecimal> = r.get("amt");
        if let Some(key) = ym {
            monthly_map.insert(key, amt.and_then(|bd| bd.to_f64()).unwrap_or(0.0));
        }
    }

    let month_names_pt = [
        "Jan", "Fev", "Mar", "Abr", "Mai", "Jun",
        "Jul", "Ago", "Set", "Out", "Nov", "Dez",
    ];

    let mut expenses_by_month = Vec::new();
    let today = now.date_naive();

    for i in (0..6).rev() {
        let mut year = today.year();
        let mut month = today.month() as i32 - i;
        while month <= 0 {
            month += 12;
            year -= 1;
        }

        let ym_key = format!("{:04}-{:02}", year, month);
        let month_name = month_names_pt.get((month - 1) as usize).unwrap_or(&"");
        let yy_short = format!("{:02}", year % 100);
        let month_label = format!("{}/{}", month_name, yy_short);

        let amt = monthly_map.get(&ym_key).copied().unwrap_or(0.0);
        expenses_by_month.push(ExpenseByMonth {
            month: month_label,
            amount: amt,
        });
    }

    // 8. Critical Activities (pending, overdue or due soon) - Top 5
    let critical_act_rows = sqlx::query(
        r#"
        SELECT a.id, a.title, a.due_date, p.name as project_name
        FROM activities a
        JOIN projects p ON p.id = a.project_id
        WHERE a.tenant_id = $1 AND a.status = 'pending'
        ORDER BY a.due_date ASC NULLS LAST
        LIMIT 5
        "#,
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let critical_activities = critical_act_rows
        .into_iter()
        .map(|r| {
            let due_date: Option<DateTime<Utc>> = r.get("due_date");
            CriticalActivity {
                id: r.get("id"),
                title: r.get("title"),
                due_date,
                project_name: r.get("project_name"),
                overdue: due_date.map(|dd| dd < now).unwrap_or(false),
            }
        })
        .collect();

    // 9. Top Companies (by contacts count) - Top 5
    let top_comp_rows = sqlx::query(
        r#"
        SELECT comp.name, COUNT(c.id) as cnt
        FROM companies comp
        JOIN contacts c ON c.company_id = comp.id
        WHERE comp.tenant_id = $1
        GROUP BY comp.name
        ORDER BY cnt DESC
        LIMIT 5
        "#,
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(AppError::from)?;

    let top_companies = top_comp_rows
        .into_iter()
        .map(|r| {
            let name: Option<String> = r.get("name");
            let cnt: i64 = r.get("cnt");
            TopCompany {
                name: name.unwrap_or_default(),
                contacts_count: cnt,
            }
        })
        .collect();

    let stats = DashboardStats {
        total_expense_amount,
        companies_count,
        contacts_count,
        leads_count,
        projects_count,
        activities_count,
        expenses_by_project,
        expenses_by_month,
        critical_activities,
        top_companies,
    };

    Ok(Json(stats))
}
