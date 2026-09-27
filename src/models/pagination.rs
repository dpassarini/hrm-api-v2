use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

impl PaginationQuery {
    pub fn page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn per_page(&self) -> i64 {
        self.per_page.unwrap_or(10).max(1).min(100)
    }

    pub fn offset(&self) -> i64 {
        (self.page() - 1) * self.per_page()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationMeta {
    pub total_pages: i64,
    pub total_count: i64,
}

impl PaginationMeta {
    pub fn new(total_count: i64, per_page: i64) -> Self {
        let total_pages = if total_count == 0 {
            0
        } else {
            (total_count as f64 / per_page as f64).ceil() as i64
        };
        PaginationMeta {
            total_pages,
            total_count,
        }
    }
}
