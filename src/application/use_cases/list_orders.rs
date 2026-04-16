use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::application::dtos::caller::Caller;
use crate::application::dtos::order_filter::OrderFilter;
use crate::application::dtos::order_output::OrderView;
use crate::application::dtos::order_sort::OrderSort;
use crate::application::errors::AppError;
use crate::domain::repositories::{MaterialRepository, OrderRepository, UserRepository};

pub struct ListOrdersUseCase<O, U, M>
where
    O: OrderRepository,
    U: UserRepository,
    M: MaterialRepository,
{
    orders: Arc<O>,
    users: Arc<U>,
    materials: Arc<M>,
}

impl<O, U, M> ListOrdersUseCase<O, U, M>
where
    O: OrderRepository,
    U: UserRepository,
    M: MaterialRepository,
{
    pub fn new(orders: Arc<O>, users: Arc<U>, materials: Arc<M>) -> Self
    {
        Self { orders, users, materials }
    }

    /// Lists orders without any filtering or sorting.
    /// Used by student "my orders" page.
    pub fn execute(&self, caller: &Caller) -> Result<Vec<OrderView>, AppError>
    {
        self.execute_filtered(caller, &OrderFilter::default(), OrderSort::default_recent())
    }

    /// Lists orders with filtering and sorting applied in application layer.
    /// Used by admin dashboard with URL query params.
    pub fn execute_filtered(
        &self,
        caller: &Caller,
        filter: &OrderFilter,
        sort: OrderSort,
    ) -> Result<Vec<OrderView>, AppError>
    {
        let orders = match caller
        {
            Caller::Admin => self.orders.find_all()?,
            Caller::Student { user_id } => self.orders.find_by_user(*user_id)?,
        };

        let user_ids: Vec<i64> = orders
            .iter()
            .map(|o| o.user_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let material_ids: Vec<i64> = orders
            .iter()
            .filter_map(|o| o.material_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let users = self.users.find_by_ids(&user_ids)?;
        let user_map: HashMap<i64, String> = users
            .into_iter()
            .map(|u| (u.id, u.display_name))
            .collect();

        let materials = self.materials.find_by_ids(&material_ids)?;
        let material_map: HashMap<i64, String> = materials
            .into_iter()
            .map(|m| (m.id, m.label()))
            .collect();

        let mut views: Vec<OrderView> = orders
            .iter()
            .map(|order|
            {
                let display_name = user_map
                    .get(&order.user_id)
                    .cloned()
                    .unwrap_or_else(|| format!("user #{}", order.user_id));
                let material_label = order
                    .material_id
                    .and_then(|mid| material_map.get(&mid).cloned());
                OrderView::from_order(order, display_name, material_label, Vec::new())
            })
            .filter(|v| filter.matches(v))
            .collect();

        views.sort_by(|a, b| sort.compare(a, b));
        Ok(views)
    }
}