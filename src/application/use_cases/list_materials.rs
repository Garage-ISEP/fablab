use std::sync::Arc;

use crate::application::dtos::order_output::MaterialView;
use crate::application::errors::AppError;
use crate::domain::material::Material;
use crate::domain::repositories::{MaterialRepository, OrderRepository, PrinterTelemetry};

pub struct ListMaterialsUseCase<M, O>
where
    M: MaterialRepository,
    O: OrderRepository,
{
    materials: Arc<M>,
    orders: Arc<O>,
    telemetry: Arc<dyn PrinterTelemetry>,
}

impl<M, O> ListMaterialsUseCase<M, O>
where
    M: MaterialRepository,
    O: OrderRepository,
{
    pub fn new(materials: Arc<M>, orders: Arc<O>, telemetry: Arc<dyn PrinterTelemetry>) -> Self {
        Self {
            materials,
            orders,
            telemetry,
        }
    }

    /// Returns raw domain materials, with no stock data. Used by the
    /// student order form where remaining-stock is irrelevant.
    pub fn execute(&self, available_only: bool) -> Result<Vec<Material>, AppError> {
        if available_only {
            self.materials.find_available().map_err(AppError::from)
        } else {
            self.materials.find_all().map_err(AppError::from)
        }
    }

    /// Returns materials enriched with the AMS measure and the weight
    /// reserved by queued orders. Each material triggers one aggregated
    /// SQL query; we accept that because the materials table is small
    /// (tens of rows at most) and the index on `orders.material_id` keeps
    /// each query fast.
    pub fn execute_with_stock(&self, available_only: bool) -> Result<Vec<MaterialView>, AppError> {
        let materials = self.execute(available_only)?;
        let inventory = self.telemetry.snapshot();

        let mut views = Vec::with_capacity(materials.len());
        for mat in &materials {
            let reserved = self.orders.sum_reserved_weight_by_material(mat.id, None)?;
            views.push(MaterialView::from_material(mat, &inventory, reserved));
        }
        Ok(views)
    }
}
