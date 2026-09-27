use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{company::Company, contact::Contact, lead::Lead},
};

pub struct LeadConversionService;

impl LeadConversionService {
    pub async fn convert(
        pool: &PgPool,
        lead_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<(Company, Contact, Lead), AppError> {
        let mut tx = pool.begin().await.map_err(AppError::from)?;

        // 1. Fetch Lead
        let lead = sqlx::query_as::<_, Lead>(
            r#"
            SELECT id, name, company_name, email, phone, job_title, website, city, state,
                   source, status, notes, categories, responsible_id, tenant_id, created_at, updated_at
            FROM leads
            WHERE id = $1 AND tenant_id = $2
            FOR UPDATE
            "#,
        )
        .bind(lead_id)
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("Lead not found".to_string()))?;

        if lead.status == "converted" {
            return Err(AppError::UnprocessableEntityMsg("Lead já foi convertido".to_string()));
        }

        let now = Utc::now();
        let target_company_name = lead
            .company_name
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&lead.name);

        let cats_to_apply = match &lead.categories {
            Some(cats) if !cats.is_empty() => cats.clone(),
            _ => vec!["Geral".to_string()],
        };

        // Ensure categories exist in the tenant
        for cat_name in &cats_to_apply {
            let cat_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO categories (id, name, tenant_id, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $4)
                ON CONFLICT (tenant_id, name) DO NOTHING
                "#,
            )
            .bind(cat_id)
            .bind(cat_name)
            .bind(tenant_id)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(AppError::from)?;
        }

        // 2. Find or create Company
        let existing_company = sqlx::query_as::<_, Company>(
            r#"
            SELECT id, name, cnpj, website, categories, tenant_id, created_at, updated_at
            FROM companies
            WHERE tenant_id = $1 AND name = $2
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(target_company_name)
        .fetch_optional(&mut *tx)
        .await
        .map_err(AppError::from)?;

        let company = match existing_company {
            Some(mut comp) => {
                let mut merged_cats = comp.categories.unwrap_or_default();
                for c in &cats_to_apply {
                    if !merged_cats.contains(c) {
                        merged_cats.push(c.clone());
                    }
                }

                let mut updated_website = comp.website.clone();
                if updated_website.as_deref().unwrap_or("").is_empty() && lead.website.is_some() {
                    updated_website = lead.website.clone();
                }

                sqlx::query(
                    r#"
                    UPDATE companies
                    SET categories = $1, website = $2, updated_at = $3
                    WHERE id = $4
                    "#,
                )
                .bind(&merged_cats)
                .bind(&updated_website)
                .bind(now)
                .bind(comp.id)
                .execute(&mut *tx)
                .await
                .map_err(AppError::from)?;

                comp.categories = Some(merged_cats);
                comp.website = updated_website;
                comp.updated_at = Some(now);
                comp
            }
            None => {
                let comp_id = Uuid::new_v4();
                sqlx::query_as::<_, Company>(
                    r#"
                    INSERT INTO companies (id, name, website, categories, tenant_id, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5, $6, $6)
                    RETURNING id, name, cnpj, website, categories, tenant_id, created_at, updated_at
                    "#,
                )
                .bind(comp_id)
                .bind(target_company_name)
                .bind(&lead.website)
                .bind(&cats_to_apply)
                .bind(tenant_id)
                .bind(now)
                .fetch_one(&mut *tx)
                .await
                .map_err(AppError::from)?
            }
        };

        // 3. Create Contact
        let contact_id = Uuid::new_v4();
        let contact = sqlx::query_as::<_, Contact>(
            r#"
            INSERT INTO contacts (id, company_id, name, email, phone, job_title, tenant_id, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
            RETURNING id, company_id, name, email, phone, job_title, tenant_id, created_at, updated_at
            "#,
        )
        .bind(contact_id)
        .bind(company.id)
        .bind(&lead.name)
        .bind(&lead.email)
        .bind(&lead.phone)
        .bind(&lead.job_title)
        .bind(tenant_id)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::from)?;

        // 4. Update Lead status
        let updated_lead = sqlx::query_as::<_, Lead>(
            r#"
            UPDATE leads
            SET status = 'converted', updated_at = $1
            WHERE id = $2
            RETURNING id, name, company_name, email, phone, job_title, website, city, state,
                      source, status, notes, categories, responsible_id, tenant_id, created_at, updated_at
            "#,
        )
        .bind(now)
        .bind(lead.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::from)?;

        tx.commit().await.map_err(AppError::from)?;

        Ok((company, contact, updated_lead))
    }
}
