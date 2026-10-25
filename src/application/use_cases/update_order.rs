use std::str::FromStr;
use std::sync::Arc;

use crate::application::dtos::caller::Caller;
use crate::application::dtos::order_input::UpdateOrderInput;
use crate::application::dtos::order_output::OrderView;
use crate::application::errors::AppError;
use crate::application::use_cases::order_files::PurgeOrderFilesUseCase;
use crate::domain::errors::DomainError;
use crate::domain::order::{Order, OrderStatus};
use crate::domain::repositories::{
    MaterialRepository, OrderFileRepository, OrderRepository, PrinterTelemetry, UserRepository,
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
    telemetry: Arc<dyn PrinterTelemetry>,
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
        telemetry: Arc<dyn PrinterTelemetry>,
    ) -> Self {
        Self {
            orders,
            users,
            materials,
            purge,
            telemetry,
        }
    }

    pub async fn execute(
        &self,
        input: UpdateOrderInput,
        caller: &Caller,
    ) -> Result<OrderView, AppError> {
        if !caller.is_admin() {
            return Err(AppError::NotAuthorized);
        }
        input.validate()?;

        let mut order = self
            .orders
            .find_by_id(input.order_id)?
            .ok_or_else(|| AppError::NotFound(format!("order {}", input.order_id)))?;

        let previous_status = order.status;
        let material_changed = input
            .material_id
            .is_some_and(|mid| Some(mid) != order.material_id);
        let weight_changed = input
            .sliced_weight_grams
            .is_some_and(|w| Some(w) != order.sliced_weight_grams);

        // Re-saving the current material is fine even once its spool is
        // gone from the AMS; only switching to an unloaded one is refused.
        if let Some(new_mid) = input.material_id.filter(|_| material_changed) {
            let material = self
                .materials
                .find_by_id(new_mid)?
                .ok_or_else(|| AppError::NotFound(format!("material {new_mid}")))?;
            if !material.available {
                return Err(AppError::InvalidInput(
                    "le materiau selectionne n'est pas disponible".to_owned(),
                ));
            }
            order.material_id = Some(new_mid);
        }

        if let Some(ref status_str) = input.status {
            let new_status = OrderStatus::from_str(status_str)?;
            order.status = order.try_advance_status(new_status)?;
        }

        if let Some(rp) = input.requires_payment {
            order.requires_payment = rp;
        }
        if let Some(w) = input.sliced_weight_grams {
            order.sliced_weight_grams = Some(w);
        }
        if let Some(t) = input.print_time_minutes {
            order.print_time_minutes = Some(t);
        }

        // Only a new reservation (weight or material set or changed on an
        // order not printed yet) needs a fresh AMS measure: other edits
        // must keep working while the printers are offline.
        match (order.material_id, order.sliced_weight_grams) {
            (Some(mid), Some(_))
                if order.status.reserves_filament() && (material_changed || weight_changed) =>
            {
                let measured = self.measured_remaining_grams(mid)?;
                self.orders.update_if_stock_sufficient(&order, measured)?;
            }
            _ => {
                self.orders.update(&order)?;
            }
        }

        let became_terminal = previous_status != order.status
            && matches!(order.status, OrderStatus::Livre | OrderStatus::Annule);
        if became_terminal {
            self.purge.execute(order.id).await?;
        }

        self.build_view(order)
    }

    fn measured_remaining_grams(&self, material_id: i64) -> Result<f64, AppError> {
        let material = self
            .materials
            .find_by_id(material_id)?
            .ok_or_else(|| AppError::NotFound(format!("material {material_id}")))?;
        let inventory = self.telemetry.snapshot();
        material
            .bambu_key
            .as_deref()
            .and_then(|key| inventory.measured_remaining_grams(key))
            .ok_or_else(|| DomainError::StockUnknown { material_id }.into())
    }

    fn build_view(&self, order: Order) -> Result<OrderView, AppError> {
        let user = self
            .users
            .find_by_id(order.user_id)?
            .ok_or_else(|| AppError::NotFound(format!("user {}", order.user_id)))?;
        let material_label = match order.material_id {
            Some(mid) => self.materials.find_by_id(mid)?.map(|m| m.label()),
            None => None,
        };
        Ok(OrderView::from_order(
            &order,
            user.display_name,
            material_label,
            Vec::new(),
        ))
    }
}
