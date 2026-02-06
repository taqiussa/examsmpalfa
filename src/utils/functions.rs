use chrono::{Datelike, Local};
use serde_json::json;
use tera::{Function, Result as TeraResult, Value};

pub struct TahunOptions;

impl Function for TahunOptions {
    fn call(&self, _args: &std::collections::HashMap<String, Value>) -> TeraResult<Value> {
        let start = 2021;

        let now = Local::now();
        let year = now.year();
        let month = now.month();

        let current_start = if month >= 7 { year } else { year - 1 };

        let mut options = vec![];

        for y in start..=current_start {
            options.push(json!({
                "value": format!("{} / {}", y, y + 1),
                "label": format!("{} / {}", y, y + 1)
            }));
        }

        Ok(json!(options))
    }
}

#[cfg(test)]
mod tests {
    use super::TahunOptions;
    use chrono::{Datelike, Local};
    use tera::Function;

    #[test]
    fn tahun_options_includes_current_year() {
        let now = Local::now();
        let year = now.year();
        let month = now.month();
        let current_start = if month >= 7 { year } else { year - 1 };

        let result = TahunOptions
            .call(&std::collections::HashMap::new())
            .unwrap();
        let arr = result.as_array().expect("expected array");
        assert!(!arr.is_empty());

        let last = arr.last().unwrap();
        let value = last.get("value").and_then(|v| v.as_str()).unwrap();
        assert_eq!(value, format!("{} / {}", current_start, current_start + 1));
    }
}
