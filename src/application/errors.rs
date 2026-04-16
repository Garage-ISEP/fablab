use crate::domain::errors::DomainError;

#[derive(Debug, thiserror::Error)]
pub enum AppError
{
    #[error("not found: {0}")]
    NotFound(String),

    #[error("not authorized")]
    NotAuthorized,

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("database error: {0}")]
    Database(String),
}

impl From<DomainError> for AppError
{
    fn from(err: DomainError) -> Self
    {
        match err
        {
            DomainError::OrderNotFound { id } => AppError::NotFound(format!("order {id}")),
            DomainError::MaterialNotFound { id } => AppError::NotFound(format!("material {id}")),
            DomainError::UserNotFound { id } => AppError::NotFound(format!("user {id}")),
            DomainError::AdminNotFound { login } => AppError::NotFound(format!("admin {login}")),
            DomainError::InvalidStatusTransition { from, to } =>
                AppError::InvalidInput(format!("invalid status transition: {from} -> {to}")),
            DomainError::Validation(msg) => AppError::InvalidInput(msg),
            DomainError::Database(msg) => AppError::Database(msg),
        }
    }
}
