use serde::{Deserialize, Serialize};

use crate::domain::filament::InventorySnapshot;
use crate::domain::material::Material;
use crate::domain::order::Order;
use crate::domain::order_file::OrderFile;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderFileView {
    pub id: i64,
    pub original_filename: String,
    pub size_bytes: i64,
}

impl OrderFileView {
    pub fn from_file(f: &OrderFile) -> Self {
        Self {
            id: f.id,
            original_filename: f.original_filename.clone(),
            size_bytes: f.size_bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderView {
    pub id: i64,
    pub user_id: i64,
    pub user_display_name: String,
    pub created_at: String,
    pub files: Vec<OrderFileView>,
    pub software_used: String,
    pub material_id: Option<i64>,
    pub material_label: Option<String>,
    pub quantity: i32,
    pub comments: Option<String>,
    pub status: String,
    pub requires_payment: bool,
    pub sliced_weight_grams: Option<f64>,
    pub print_time_minutes: Option<i32>,
}

impl OrderView {
    pub fn from_order(
        order: &Order,
        user_display_name: String,
        material_label: Option<String>,
        files: Vec<OrderFileView>,
    ) -> Self {
        Self {
            id: order.id,
            user_id: order.user_id,
            user_display_name,
            created_at: order.created_at.clone(),
            files,
            software_used: order.software_used.clone(),
            material_id: order.material_id,
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

/// Admin-facing material view enriched with stock data from the AMS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialView {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub color_hex: Option<String>,
    pub label: String,
    /// Currently loaded in an AMS.
    pub available: bool,
    /// Where it is loaded, e.g. "X1C B3 (9 %)".
    pub locations: Vec<String>,
    /// `None` when no fresh printer reports a usable amount.
    pub measured_remaining_grams: Option<f64>,
    /// Sliced weight of the orders not printed yet.
    pub reserved_grams: f64,
    /// Measured minus reserved; `None` when the stock is unknown.
    pub available_grams: Option<f64>,
}

impl MaterialView {
    pub fn from_material(
        material: &Material,
        inventory: &InventorySnapshot,
        reserved_grams: f64,
    ) -> Self {
        let key = material.bambu_key.as_deref();
        let measured = key.and_then(|k| inventory.measured_remaining_grams(k));
        Self {
            id: material.id,
            name: material.name.clone(),
            color: material.color.clone(),
            color_hex: material.color_hex.clone(),
            label: material.label(),
            available: material.available,
            locations: key.map(|k| inventory.locations(k)).unwrap_or_default(),
            measured_remaining_grams: measured,
            reserved_grams,
            available_grams: measured
                .map(|grams| crate::domain::stock::available_weight(grams, reserved_grams)),
        }
    }
}
