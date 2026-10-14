use crate::models::role::Role;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountType {
    Web,
    Siswa,
}

impl AccountType {
    pub fn cookie_value(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Siswa => "siswa",
        }
    }

    pub fn from_cookie(value: &str) -> Option<Self> {
        match value {
            "web" => Some(Self::Web),
            "siswa" => Some(Self::Siswa),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct AuthUser {
    #[allow(dead_code)]
    pub id: u64,
    pub account_type: AccountType,
    pub nis: Option<String>,
    pub roles: Vec<Role>,
    pub name: String,
    pub foto: Option<String>,
}
