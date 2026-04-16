use std::sync::Arc;

use crate::application::errors::AppError;
use crate::domain::material::Material;
use crate::domain::repositories::MaterialRepository;

pub struct ListMaterialsUseCase<M: MaterialRepository>
{
    materials: Arc<M>,
}

impl<M: MaterialRepository> ListMaterialsUseCase<M>
{
    pub fn new(materials: Arc<M>) -> Self
    {
        Self { materials }
    }

    pub fn execute(&self, available_only: bool) -> Result<Vec<Material>, AppError>
    {
        if available_only
        {
            self.materials.find_available().map_err(AppError::from)
        }
        else
        {
            self.materials.find_all().map_err(AppError::from)
        }
    }
}
