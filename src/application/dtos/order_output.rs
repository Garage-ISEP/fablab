use serde::{Deserialize, Serialize};

use crate::domain::order::Order;
use crate::domain::order_file::OrderFile;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderFileView
{
    pub id: i64,
    pub original_filename: String,
    pub size_bytes: i64,
}

impl OrderFileView
{
    pub fn from_file(f: &OrderFile) -> Self
    {
        Self
        {
            id: f.id,
            original_filename: f.original_filename.clone(),
            size_bytes: f.size_bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderView
{
    pub id: i64,
    pub user_id: i64,
    pub user_display_name: String,
    pub created_at: String,
    pub files: Vec<OrderFileView>,
    pub software_used: String,
    pub material_label: Option<String>,
    pub quantity: i32,
    pub comments: Option<String>,
    pub status: String,
    pub requires_payment: bool,
    pub sliced_weight_grams: Option<f64>,
    pub print_time_minutes: Option<i32>,
}

impl OrderView
{
    pub fn from_order(
        order: &Order,
        user_display_name: String,
        material_label: Option<String>,
        files: Vec<OrderFileView>,
    ) -> Self
    {
        Self
        {
            id: order.id,
            user_id: order.user_id,
            user_display_name,
            created_at: order.created_at.clone(),
            files,
            software_used: order.software_used.clone(),
            material_label,
            quantity: order.quantity,
            comments: order.comments.clone(),
            status: order.status.as_str().to_owned(),
            requires_payment: order.requires_payment,
            sliced_weight_grams: order.sliced_weight_grams,
            print_time_minutes: order.print_time_minutes,
        }
    }
}
