use std::sync::Arc;

use crate::application::errors::AppError;
use crate::domain::material::Material;
use crate::domain::repositories::MaterialRepository;

pub struct ManageMaterialUseCase<M>
where
    M: MaterialRepository,
{
    materials: Arc<M>,
}

impl<M> ManageMaterialUseCase<M>
where
    M: MaterialRepository,
{
    pub fn new(materials: Arc<M>) -> Self
    {
        Self { materials }
    }

    pub fn execute(&self, material: Material) -> Result<(), AppError>
    {
        self.materials.upsert(&material)?;
        Ok(())
    }

    pub fn next_id(&self) -> Result<i64, AppError>
    {
        let max = self.materials.max_id()?;
        Ok(max + 1)
    }

    pub fn delete(&self, id: i64) -> Result<(), AppError>
    {
        let in_use = self.materials.count_orders_using(id)?;
        if in_use > 0
        {
            return Err(AppError::InvalidInput(
                "material is referenced by orders".to_owned(),
            ));
        }
        self.materials.delete(id)?;
        Ok(())
    }
}
