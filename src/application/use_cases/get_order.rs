use std::sync::Arc;

use crate::application::dtos::caller::Caller;
use crate::application::dtos::order_output::{OrderFileView, OrderView};
use crate::application::errors::AppError;
use crate::domain::repositories::
{
    MaterialRepository, OrderFileRepository, OrderRepository, UserRepository,
};

pub struct GetOrderUseCase<O, U, M, F>
where
    O: OrderRepository,
    U: UserRepository,
    M: MaterialRepository,
    F: OrderFileRepository,
{
    orders: Arc<O>,
    users: Arc<U>,
    materials: Arc<M>,
    files: Arc<F>,
}

impl<O, U, M, F> GetOrderUseCase<O, U, M, F>
where
    O: OrderRepository,
    U: UserRepository,
    M: MaterialRepository,
    F: OrderFileRepository,
{
    pub fn new(orders: Arc<O>, users: Arc<U>, materials: Arc<M>, files: Arc<F>) -> Self
    {
        Self { orders, users, materials, files }
    }

    pub fn execute(&self, order_id: i64, caller: &Caller) -> Result<OrderView, AppError>
    {
        let order = self.orders
            .find_by_id(order_id)?
            .ok_or_else(|| AppError::NotFound(format!("order {order_id}")))?;

        if let Caller::Student { user_id } = caller
            && order.user_id != *user_id
        {
            return Err(AppError::NotAuthorized);
        }

        let user = self.users
            .find_by_id(order.user_id)?
            .ok_or_else(|| AppError::NotFound(format!("user {}", order.user_id)))?;

        let material_label = match order.material_id
        {
            Some(mid) => self.materials.find_by_id(mid)?.map(|m| m.label()),
            None => None,
        };

        let files: Vec<OrderFileView> = self.files
            .find_by_order(order.id)?
            .iter()
            .map(OrderFileView::from_file)
            .collect();

        Ok(OrderView::from_order(&order, user.display_name, material_label, files))
    }
}
