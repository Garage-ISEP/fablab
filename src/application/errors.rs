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
            DomainError::InsufficientStock { material_id, requested_grams, available_grams } =>
                AppError::InvalidInput(format!(
                    "stock insuffisant pour le materiau {material_id}: \
                     demande {requested_grams:.1}g, disponible {available_grams:.1}g"
                )),
            DomainError::MaterialRequiredForStatus { target } =>
                AppError::InvalidInput(format!(
                    "un materiau doit etre defini avant de passer la commande au statut '{target}'"
                )),
            DomainError::Validation(msg) => AppError::InvalidInput(msg),
            DomainError::Database(msg) => AppError::Database(msg),
        }
    }
}
