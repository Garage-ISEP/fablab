use std::sync::Arc;

use crate::application::dtos::order_input::SubmitOrderInput;
use crate::application::errors::AppError;
use crate::application::validation;
use crate::domain::order::NewOrder;
use crate::domain::repositories::{MaterialRepository, OrderRepository, UserRepository};

pub struct SubmitOrderUseCase<O, U, M>
where
    O: OrderRepository,
    U: UserRepository,
    M: MaterialRepository,
{
    orders: Arc<O>,
    users: Arc<U>,
    materials: Arc<M>,
}

impl<O, U, M> SubmitOrderUseCase<O, U, M>
where
    O: OrderRepository,
    U: UserRepository,
    M: MaterialRepository,
{
    pub fn new(orders: Arc<O>, users: Arc<U>, materials: Arc<M>) -> Self
    {
        Self { orders, users, materials }
    }

    /// Validates the input, persists a new order row, optionally updates
    /// the caller's phone number, and returns the freshly created order
    /// id. Files must be attached via UploadOrderFileUseCase before the
    /// caller considers the submission complete.
    pub fn execute(&self, input: SubmitOrderInput) -> Result<i64, AppError>
    {
        let software_used = validation::sanitize_text(&input.software_used);
        if software_used.is_empty()
        {
            return Err(AppError::InvalidInput("software_used is required".to_owned()));
        }

        if input.quantity < 1
        {
            return Err(AppError::InvalidInput("quantity must be at least 1".to_owned()));
        }

        if let Some(mid) = input.material_id
        {
            let mat = self.materials.find_by_id(mid)?;
            if mat.is_none()
            {
                return Err(AppError::InvalidInput("material not found".to_owned()));
            }
        }

        let phone_raw = input.phone.as_deref().unwrap_or("").trim();
        if phone_raw.is_empty()
        {
            return Err(AppError::InvalidInput("phone is required".to_owned()));
        }
        let phone = validation::validate_phone(phone_raw)?;
        self.users.update_phone(input.user_id, &phone)?;

        let comments = input.comments
            .as_deref()
            .map(validation::sanitize_text)
            .filter(|s| !s.is_empty());

        let new_order = NewOrder
        {
            user_id: input.user_id,
            software_used,
            material_id: input.material_id,
            quantity: input.quantity,
            comments,
        };

        let order = self.orders.create(new_order)?;
        Ok(order.id)
    }
}
