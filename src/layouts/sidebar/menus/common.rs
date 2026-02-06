use crate::layouts::sidebar::sidebar::SidebarItem;
use crate::utils::route_matcher::is_active;

pub fn menu(current_path: &str) -> Vec<SidebarItem> {
    vec![
        SidebarItem {
            kind: "link",
            href: "/dashboard",
            label: "Dashboard",
            active: is_active(current_path, "/dashboard"),
        },
        SidebarItem {
            kind: "link",
            href: "/profil",
            label: "Profile",
            active: is_active(current_path, "/profil"),
        },
    ]
}
