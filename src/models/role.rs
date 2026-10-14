#[derive(Clone, PartialEq, Eq)]
pub enum Role {
    Admin,
    Guru,
    Konseling,
    Guest, // represents unauthenticated state
}

#[allow(dead_code)]
impl Role {
    pub fn from(s: &str) -> Self {
        match s {
            "Admin" => Role::Admin,
            "Guru" => Role::Guru,
            "Konseling" => Role::Konseling,
            _ => Role::Guest,
        }
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
        assert!(matches!(admin, Role::Admin));
        let siswa = Role::from("Siswa");
        assert!(siswa.is_guest());
        let unknown = Role::from("Unknown");
        assert!(unknown.is_guest());
    }
}
