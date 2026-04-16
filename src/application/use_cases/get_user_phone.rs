use std::sync::Arc;

use crate::application::errors::AppError;
use crate::domain::repositories::UserRepository;

pub struct GetUserPhoneUseCase<U: UserRepository>
{
    users: Arc<U>,
}

impl<U: UserRepository> GetUserPhoneUseCase<U>
{
    pub fn new(users: Arc<U>) -> Self
    {
        Self { users }
    }

    pub fn execute(&self, user_id: i64) -> Result<Option<String>, AppError>
    {
        let user = self.users.find_by_id(user_id)?;
        Ok(user.and_then(|u| u.phone))
    }
}
