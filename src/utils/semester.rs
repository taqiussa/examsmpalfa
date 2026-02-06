use chrono::{Datelike, Local};

pub fn data_semester() -> i32 {
    let now = Local::now();
    let month = now.month();
    let day = now.day();

    if month > 7 || (month == 7 && day >= 12) {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::data_semester;
    use chrono::{Datelike, Local};

    #[test]
    fn data_semester_matches_date_logic() {
        let now = Local::now();
        let month = now.month();
        let day = now.day();

        let expected = if month > 7 || (month == 7 && day >= 12) {
            1
        } else {
            2
        };

        assert_eq!(data_semester(), expected);
    }
}
