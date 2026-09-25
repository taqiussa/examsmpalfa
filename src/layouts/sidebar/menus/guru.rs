use crate::layouts::sidebar::sidebar::SidebarItem;
use crate::utils::route_matcher::is_active;

pub fn menu(current_path: &str) -> Vec<SidebarItem> {
    vec![
        SidebarItem {
            kind: "link",
            href: "/ujian",
            label: "Buat Ujian",
            active: is_active(current_path, "/ujian"),
        },
        SidebarItem {
            kind: "link",
            href: "/cetak-kartu",
            label: "Cetak Kartu",
            active: is_active(current_path, "/cetak-kartu"),
        },
        SidebarItem {
            kind: "link",
            href: "/data-kelas",
            label: "Data Kelas",
            active: is_active(current_path, "/data-kelas"),
        },
        SidebarItem {
            kind: "link",
            href: "/data-peserta",
            label: "Data Peserta",
            active: is_active(current_path, "/data-peserta"),
        },
        SidebarItem {
            kind: "link",
            href: "/hasil-nilai",
            label: "Hasil Nilai",
            active: is_active(current_path, "/hasil-nilai"),
        },
        SidebarItem {
            kind: "link",
            href: "/mata-pelajaran",
            label: "Mata Pelajaran",
            active: is_active(current_path, "/mata-pelajaran"),
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
            href: "/status-peserta",
            label: "Status Peserta",
            active: is_active(current_path, "/status-peserta"),
        },
        SidebarItem {
            kind: "link",
            href: "/upload-data-siswa",
            label: "Upload Data Siswa",
            active: is_active(current_path, "/upload-data-siswa"),
        },
        SidebarItem {
            kind: "link",
            href: "/upload-peserta",
            label: "Upload Peserta",
            active: is_active(current_path, "/upload-peserta"),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::menu;

    #[test]
    fn guru_menu_is_alphabetical_and_hides_absensi_kelas() {
        let items = menu("/");
        let labels: Vec<_> = items.iter().map(|item| item.label).collect();
        let mut sorted = labels.clone();
        sorted.sort_unstable();

        assert_eq!(labels, sorted);
        assert!(!labels.contains(&"Absensi Kelas"));
        assert!(labels.contains(&"Data Kelas"));
        assert!(labels.contains(&"Upload Peserta"));
    }
}
