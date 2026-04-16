use std::sync::Arc;

use crate::application::errors::AppError;
use crate::domain::repositories::UserRepository;

pub struct UpdatePhoneUseCase<U: UserRepository>
{
    users: Arc<U>,
}

impl<U: UserRepository> UpdatePhoneUseCase<U>
{
    pub fn new(users: Arc<U>) -> Self
    {
        Self { users }
    }

    pub fn execute(&self, user_id: i64, phone: &str) -> Result<(), AppError>
    {
        if phone.trim().is_empty()
        {
            return Err(AppError::InvalidInput("phone cannot be empty".to_owned()));
        }
        self.users.update_phone(user_id, phone)?;
        Ok(())
    }
}
