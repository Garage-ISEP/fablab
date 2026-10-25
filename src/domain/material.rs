use serde::{Deserialize, Serialize};

/// A filament product students can pick. Materials are derived from the
/// spools loaded in the printers' AMS: `bambu_key` identifies the product
/// (see [`crate::domain::filament::LoadedSpool::material_key`]) and
/// `available` tells whether such a spool is currently loaded. Legacy rows
/// created by hand before the AMS integration have no key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Material {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub color_hex: Option<String>,
    pub bambu_key: Option<String>,
    pub available: bool,
}

impl Material {
    /// Display label combining name and color, e.g. "PLA - Noir mat".
    pub fn label(&self) -> String {
        format!("{} - {}", self.name, self.color)
    }
}

/// What the AMS tells us about a product currently loaded somewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedMaterial {
    pub bambu_key: String,
    pub name: String,
    pub color: String,
    pub color_hex: String,
}
