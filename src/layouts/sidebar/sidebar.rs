use crate::layouts::sidebar::menus;
use crate::models::auth_user::AuthUser;
use crate::models::role::Role;
use serde::Serialize;

#[derive(Serialize)]
pub struct LayoutContext {
    pub sidebar: Vec<SidebarItem>,
    pub user: Option<LayoutUser>,
}

#[derive(Serialize)]
pub struct LayoutUser {
    pub name: String,
    pub foto: Option<String>,
}

impl LayoutContext {
    pub fn with_user(user: &AuthUser, current_path: &str) -> Self {
        Self {
            sidebar: build_sidebar(&user.roles, current_path),
            user: Some(LayoutUser {
                name: user.name.clone(),
                foto: user.foto.clone(),
            }),
        }
    }
}

#[derive(Clone, Serialize)]
pub struct SidebarItem {
    pub kind: &'static str,
    pub label: &'static str,
    pub href: &'static str,
    pub active: bool,
}

pub fn build_sidebar(roles: &[Role], current_path: &str) -> Vec<SidebarItem> {
    let mut items: Vec<SidebarItem> = Vec::new();

    let has_siswa = roles.iter().any(Role::is_siswa);

    if !has_siswa {
        items.extend(menus::common::menu(current_path));
    }

    if roles.iter().any(|r| matches!(r, Role::Admin)) {
        items.push(SidebarItem {
            kind: "header",
            label: "Admin",
            href: "",
            active: false,
        });

        items.extend(menus::admin::menu(current_path));
    }

    if roles.iter().any(|r| matches!(r, Role::Guru)) {
        items.push(SidebarItem {
            kind: "header",
            label: "Guru",
            href: "",
            active: false,
        });

        items.extend(menus::guru::menu(current_path));

        items.push(SidebarItem {
            kind: "header",
            label: "Wali Kelas",
            href: "",
            active: false,
        });

        items.extend(menus::wali_kelas::menu(current_path));
    }

    if roles.iter().any(|r| matches!(r, Role::Siswa)) {
        items.push(SidebarItem {
            kind: "header",
            label: "Siswa",
            href: "",
            active: false,
        });

        items.extend(menus::siswa::menu(current_path));
    }

    items
}

#[cfg(test)]
mod tests {
    use super::build_sidebar;
    use crate::models::role::Role;

    #[test]
    fn build_sidebar_for_admin_includes_admin_header() {
        let items = build_sidebar(&[Role::Admin], "/dashboard");
        assert!(items.iter().any(|i| i.kind == "header" && i.label == "Admin"));
        assert!(items.iter().any(|i| i.href == "/dashboard" && i.active));
    }

    #[test]
    fn build_sidebar_for_siswa_only_has_siswa_section() {
        let items = build_sidebar(&[Role::Siswa], "/siswa");
        assert!(items.iter().any(|i| i.kind == "header" && i.label == "Siswa"));
        assert!(!items.iter().any(|i| i.kind == "header" && i.label == "Admin"));
    }
}
