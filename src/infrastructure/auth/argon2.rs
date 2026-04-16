use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

use crate::domain::errors::DomainError;

pub fn hash_password(plain: &str) -> Result<String, DomainError>
{
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let hash = argon2
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| DomainError::Validation(format!("password hash error: {e}")))?;

    Ok(hash.to_string())
}

pub struct Argon2PasswordVerifier;

impl crate::domain::repositories::PasswordVerifier for Argon2PasswordVerifier
{
    fn verify(&self, plain: &str, hash: &str) -> Result<bool, DomainError>
    {
        let parsed = PasswordHash::new(hash)
            .map_err(|e| DomainError::Validation(format!("invalid hash format: {e}")))?;

        let argon2 = Argon2::default();
        Ok(argon2.verify_password(plain.as_bytes(), &parsed).is_ok())
    }
}
