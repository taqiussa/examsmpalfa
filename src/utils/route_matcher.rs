pub fn is_active(current_path: &str, route: &str) -> bool {
    if route == "/" {
        return current_path == "/";
    }

    current_path == route || current_path.starts_with(&format!("{}/", route))
}

#[cfg(test)]
mod tests {
    use super::is_active;

    #[test]
    fn is_active_handles_root_and_prefix() {
        assert!(is_active("/", "/"));
        assert!(!is_active("/dashboard", "/"));
        assert!(is_active("/dashboard", "/dashboard"));
        assert!(is_active("/dashboard/settings", "/dashboard"));
        assert!(!is_active("/profil", "/dashboard"));
    }
}
