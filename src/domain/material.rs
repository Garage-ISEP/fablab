use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Material
{
    pub id: i64,
    pub name: String,
    pub color: String,
    pub available: bool,
}

impl Material
{
    /// Display label combining name and color, e.g. "PLA - Noir mat".
    pub fn label(&self) -> String
    {
        format!("{} - {}", self.name, self.color)
    }
}