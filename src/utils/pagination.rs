use serde::Serialize;

pub const PAGE_SIZE: i64 = 10;

#[derive(Debug, Serialize, Clone)]
pub struct Pagination {
    pub page: i64,
    pub total_pages: i64,
    pub total_rows: i64,
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct PaginationConfig {
    pub url: String,
    pub target: String,
    pub extra_query: String,
    pub history_url: String,
}

pub fn build_pagination(page: i64, total_rows: i64) -> Pagination {
    let total_pages = ((total_rows as f64) / (PAGE_SIZE as f64)).ceil().max(1.0) as i64;

    let safe_page = page.clamp(1, total_pages);

    let start = if total_rows == 0 {
        0
    } else {
        (safe_page - 1) * PAGE_SIZE + 1
    };

    let end = (safe_page * PAGE_SIZE).min(total_rows);

    Pagination {
        page: safe_page,
        total_pages,
        total_rows,
        start,
        end,
    }
}

#[cfg(test)]
mod tests {
    use super::{PAGE_SIZE, build_pagination};

    #[test]
    fn build_pagination_clamps_page_and_ranges() {
        let p = build_pagination(1, 0);
        assert_eq!(p.page, 1);
        assert_eq!(p.total_pages, 1);
        assert_eq!(p.start, 0);
        assert_eq!(p.end, 0);

        let p = build_pagination(2, PAGE_SIZE + 1);
        assert_eq!(p.page, 2);
        assert_eq!(p.total_pages, 2);
        assert_eq!(p.start, PAGE_SIZE + 1);
        assert_eq!(p.end, PAGE_SIZE + 1);

        let p = build_pagination(99, PAGE_SIZE + 1);
        assert_eq!(p.page, 2);
        assert_eq!(p.total_pages, 2);
    }
}
