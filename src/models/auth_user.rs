use crate::models::role::Role;

#[derive(Clone)]
pub struct AuthUser {
    #[allow(dead_code)]
    pub id: u64,
    pub nis: Option<String>,
    pub roles: Vec<Role>, // ⬅️ multi-role sekarang
    pub name: String,
    pub foto: Option<String>,
}
