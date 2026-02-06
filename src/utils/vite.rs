use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

fn is_dev() -> bool {
    std::env::var("APP_ENV").unwrap_or_else(|_| "production".into()) == "development"
}

#[derive(Debug, Deserialize)]
struct ViteManifestEntry {
    file: String,
    css: Option<Vec<String>>,
    is_entry: Option<bool>,
}

pub fn vite_js() -> String {
    if is_dev() {
        return r#"<script type="module" src="http://localhost:5173/src/main.ts"></script>"#.into();
    }

    let manifest = load_manifest();

    // ✅ Cari entry utama dari manifest Vite
    let entry = manifest
        .get("index.html")
        .or_else(|| manifest.values().find(|e| e.is_entry.unwrap_or(false)));

    match entry {
        Some(e) => format!(
            r#"<script type="module" src="/static/{}"></script>"#,
            e.file
        ),
        None => {
            // ✅ Jangan panic di production
            // Kalau manifest hilang, minimal app tetap hidup
            "".into()
        }
    }
}

pub fn vite_css() -> String {
    if is_dev() {
        return "".into();
    }

    let manifest = load_manifest();
    let entry = manifest
        .get("index.html")
        .or_else(|| manifest.values().find(|e| e.is_entry.unwrap_or(false)));

    match entry.and_then(|e| e.css.as_ref()) {
        Some(css_files) => css_files
            .iter()
            .map(|css| format!(r#"<link rel="stylesheet" href="/static/{}">"#, css))
            .collect::<Vec<_>>()
            .join("\n"),
        None => "".into(),
    }
}

fn load_manifest() -> HashMap<String, ViteManifestEntry> {
    let path = "static/.vite/manifest.json";
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };

    serde_json::from_str(&content).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{vite_css, vite_js};
    use serial_test::serial;
    use std::fs;

    #[test]
    #[serial]
    fn vite_js_dev_mode_points_to_vite_server() {
        unsafe { std::env::set_var("APP_ENV", "development") };
        let js = vite_js();
        assert!(js.contains("http://localhost:5173/src/main.ts"));
        unsafe { std::env::remove_var("APP_ENV") };
    }

    #[test]
    #[serial]
    fn vite_css_and_js_from_manifest_in_production() {
        unsafe { std::env::set_var("APP_ENV", "production") };
        fs::create_dir_all("static/.vite").unwrap();
        fs::write(
            "static/.vite/manifest.json",
            r#"{ "index.html": { "file": "assets/main.js", "css": ["assets/main.css"], "is_entry": true } }"#,
        )
        .unwrap();

        let js = vite_js();
        let css = vite_css();
        assert!(js.contains("/static/assets/main.js"));
        assert!(css.contains("/static/assets/main.css"));

        let _ = fs::remove_file("static/.vite/manifest.json");
        let _ = fs::remove_dir("static/.vite");
        unsafe { std::env::remove_var("APP_ENV") };
    }
}
