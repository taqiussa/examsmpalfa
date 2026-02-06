#[derive(Clone, PartialEq, Eq)]
pub enum Role {
    Admin,
    Guru,
    Siswa,
    Konseling,
    Guest, // represents unauthenticated state
}

#[allow(dead_code)]
impl Role {
    pub fn from(s: &str) -> Self {
        match s {
            "Admin" => Role::Admin,
            "Guru" => Role::Guru,
            "Siswa" => Role::Siswa,
            "Konseling" => Role::Konseling,
            _ => Role::Guest,
        }
    }

    pub fn is_siswa(&self) -> bool {
        matches!(self, Role::Siswa)
    }

    pub fn is_non_siswa(&self) -> bool {
        !self.is_siswa()
    }

    pub fn is_guest(&self) -> bool {
        matches!(self, Role::Guest)
    }
}

#[cfg(test)]
mod tests {
    use super::Role;

    #[test]
    fn role_from_and_checks() {
        let admin = Role::from("Admin");
        assert!(!admin.is_siswa());
        let siswa = Role::from("Siswa");
        assert!(siswa.is_siswa());
        let unknown = Role::from("Unknown");
        assert!(unknown.is_guest());
    }
}
