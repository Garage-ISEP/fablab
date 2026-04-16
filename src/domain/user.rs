use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User
{
    pub id: i64,
    pub cas_login: String,
    pub display_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub promo: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CasUser
{
    pub cas_login: String,
    pub display_name: String,
    pub email: String,
    pub promo: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminUser
{
    pub id: i64,
    pub login: String,
    pub password_hash: String,
}
