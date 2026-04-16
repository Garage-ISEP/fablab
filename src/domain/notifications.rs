use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifiedFile
{
    pub file_id: i64,
    pub original_filename: String,
    pub size_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderNotification
{
    pub order_id: i64,
    pub created_at: String,
    pub user_display_name: String,
    pub user_email: String,
    pub user_phone: Option<String>,
    pub software_used: String,
    pub material_label: Option<String>,
    pub quantity: i32,
    pub comments: Option<String>,
    pub files: Vec<NotifiedFile>,
    pub download_base_url: String,
}