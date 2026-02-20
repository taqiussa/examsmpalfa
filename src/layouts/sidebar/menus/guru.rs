use crate::layouts::sidebar::sidebar::SidebarItem;
use crate::utils::route_matcher::is_active;

pub fn menu(current_path: &str) -> Vec<SidebarItem> {
    vec![
        SidebarItem {
            kind: "link",
            href: "/biodata-siswa",
            label: "Biodata Siswa",
            active: is_active(current_path, "/biodata-siswa"),
        },
        SidebarItem {
            kind: "link",
            href: "/absensi-kelas",
            label: "Absensi Kelas",
            active: is_active(current_path, "/absensi-kelas"),
        },
        SidebarItem {
            kind: "link",
            href: "/ujian",
            label: "Buat Ujian",
            active: is_active(current_path, "/ujian"),
        },
        SidebarItem {
            kind: "link",
            href: "/progress-ujian",
            label: "Progress Ujian",
            active: is_active(current_path, "/progress-ujian"),
        },
        SidebarItem {
            kind: "link",
            href: "/review-uraian",
            label: "Review Uraian",
            active: is_active(current_path, "/review-uraian"),
        },
        SidebarItem {
            kind: "link",
            href: "/hasil-nilai",
            label: "Hasil Nilai",
            active: is_active(current_path, "/hasil-nilai"),
        },
    ]
}
