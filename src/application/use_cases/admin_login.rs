use std::sync::Arc;

use crate::application::errors::AppError;
use crate::domain::repositories::{AdminRepository, PasswordVerifier};
use crate::domain::user::AdminUser;

pub struct AdminLoginUseCase<A, P>
where
    A: AdminRepository,
    P: PasswordVerifier,
{
    admins: Arc<A>,
    verifier: Arc<P>,
}

impl<A, P> AdminLoginUseCase<A, P>
where
    A: AdminRepository,
    P: PasswordVerifier,
{
    pub fn new(admins: Arc<A>, verifier: Arc<P>) -> Self
    {
        Self { admins, verifier }
    }

    pub fn execute(&self, login: &str, password: &str) -> Result<AdminUser, AppError>
    {
        let admin = self.admins.find_by_login(login)?.ok_or(AppError::NotAuthorized)?;
        let valid = self.verifier
            .verify(password, &admin.password_hash)
            .map_err(|e| AppError::InvalidInput(e.to_string()))?;
        if !valid
        {
            return Err(AppError::NotAuthorized);
        }
        Ok(admin)
    }
}
