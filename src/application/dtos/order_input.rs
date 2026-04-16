use serde::{Deserialize, Serialize};

use crate::application::errors::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitOrderInput
{
    pub user_id: i64,
    pub software_used: String,
    pub material_id: Option<i64>,
    pub quantity: i32,
    pub comments: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateOrderInput
{
    pub order_id: i64,
    pub status: Option<String>,
    pub requires_payment: Option<bool>,
    pub sliced_weight_grams: Option<f64>,
    pub print_time_minutes: Option<i32>,
}

impl UpdateOrderInput
{
    pub fn validate(&self) -> Result<(), AppError>
    {
        if let Some(w) = self.sliced_weight_grams
            && w < 0.0
        {
            return Err(AppError::InvalidInput("weight must be >= 0".to_owned()));
        }
        if let Some(t) = self.print_time_minutes
            && t < 0
        {
            return Err(AppError::InvalidInput("print time must be >= 0".to_owned()));
        }
        Ok(())
    }
}