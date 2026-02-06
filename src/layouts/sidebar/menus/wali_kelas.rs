use crate::layouts::sidebar::sidebar::SidebarItem;
use crate::utils::route_matcher::is_active;

pub fn menu(current_path: &str) -> Vec<SidebarItem> {
    vec![SidebarItem {
        kind: "link",
        href: "/print-rapor",
        label: "Print Rapor",
        active: is_active(current_path, "/print-rapor"),
    }]
}
