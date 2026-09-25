use chrono::{Datelike, Local};
use serde_json::json;
use tera::{Function, Result as TeraResult, Value};

pub struct TahunOptions;

pub struct LabLabel;

impl Function for LabLabel {
    fn call(&self, args: &std::collections::HashMap<String, Value>) -> TeraResult<Value> {
        let Some(kode) = args.get("kode").and_then(Value::as_str) else {
            return Ok(json!("-"));
        };

        match kode.parse::<u8>() {
            Ok(nomor @ 1..=15) => Ok(json!(format!("Lab {nomor}"))),
            _ => Ok(json!("-")),
        }
    }
}

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
    use super::{LabLabel, TahunOptions};
    use chrono::{Datelike, Local};
    use serde_json::json;
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

    #[test]
    fn lab_label_uses_plain_lab_number() {
        let args = std::collections::HashMap::from([("kode".to_string(), json!("09"))]);
        assert_eq!(LabLabel.call(&args).unwrap(), json!("Lab 9"));
    }
}
