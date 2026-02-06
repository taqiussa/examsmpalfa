use chrono::{Datelike, Local};

pub fn data_tahun() -> String {
    let now = Local::now();
    let year = now.year();
    let month = now.month();

    let (start, end) = if month >= 7 {
        (year, year + 1)
    } else {
        (year - 1, year)
    };

    format!("{} / {}", start, end)
}

#[cfg(test)]
mod tests {
    use super::data_tahun;
    use chrono::{Datelike, Local};

    #[test]
    fn data_tahun_matches_expected_range() {
        let now = Local::now();
        let year = now.year();
        let month = now.month();

        let (start, end) = if month >= 7 {
            (year, year + 1)
        } else {
            (year - 1, year)
        };

        assert_eq!(data_tahun(), format!("{} / {}", start, end));
    }
}
