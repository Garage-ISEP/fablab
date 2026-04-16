use std::str::FromStr;
use std::sync::Arc;

use crate::application::dtos::caller::Caller;
use crate::application::dtos::order_input::UpdateOrderInput;
use crate::application::dtos::order_output::OrderView;
use crate::application::errors::AppError;
use crate::application::use_cases::order_files::PurgeOrderFilesUseCase;
use crate::domain::order::OrderStatus;
use crate::domain::repositories::
{
    MaterialRepository, OrderFileRepository, OrderRepository, UserRepository,
};

pub struct UpdateOrderUseCase<O, U, M, F>
where
    O: OrderRepository,
    U: UserRepository,
    M: MaterialRepository,
    F: OrderFileRepository,
{
    orders: Arc<O>,
    users: Arc<U>,
    materials: Arc<M>,
    purge: Arc<PurgeOrderFilesUseCase<F>>,
}

impl<O, U, M, F> UpdateOrderUseCase<O, U, M, F>
where
    O: OrderRepository,
    U: UserRepository,
    M: MaterialRepository,
    F: OrderFileRepository,
{
    pub fn new(
        orders: Arc<O>,
        users: Arc<U>,
        materials: Arc<M>,
        purge: Arc<PurgeOrderFilesUseCase<F>>,
    ) -> Self
    {
        Self { orders, users, materials, purge }
    }

    pub async fn execute(
        &self,
        input: UpdateOrderInput,
        caller: &Caller,
    ) -> Result<OrderView, AppError>
    {
        if !caller.is_admin()
        {
            return Err(AppError::NotAuthorized);
        }
        input.validate()?;

        let mut order = self.orders
            .find_by_id(input.order_id)?
            .ok_or_else(|| AppError::NotFound(format!("order {}", input.order_id)))?;

        let previous_status = order.status;

        if let Some(ref status_str) = input.status
        {
            let new_status = OrderStatus::from_str(status_str)?;
            order.status = order.status.transition_to(new_status)?;
        }

        if let Some(rp) = input.requires_payment
        {
            order.requires_payment = rp;
        }
        if let Some(w) = input.sliced_weight_grams
        {
            order.sliced_weight_grams = Some(w);
        }
        if let Some(t) = input.print_time_minutes
        {
            order.print_time_minutes = Some(t);
        }

        self.orders.update(&order)?;

        // If we just transitioned into a terminal state, drop the
        // attached files. Already-terminal orders (no transition) are
        // left untouched: their files were purged at the original
        // transition.
        let became_terminal = previous_status != order.status
            && matches!(order.status, OrderStatus::Livre | OrderStatus::Annule);
        if became_terminal
        {
            self.purge.execute(order.id).await?;
        }

        let user = self.users
            .find_by_id(order.user_id)?
            .ok_or_else(|| AppError::NotFound(format!("user {}", order.user_id)))?;
        let material_label = match order.material_id
        {
            Some(mid) => self.materials.find_by_id(mid)?.map(|m| m.label()),
            None => None,
        };
        Ok(OrderView::from_order(&order, user.display_name, material_label, Vec::new()))
    }
}
