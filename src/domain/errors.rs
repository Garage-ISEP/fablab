#[derive(Debug, thiserror::Error)]
pub enum DomainError
{
    #[error("order not found: {id}")]
    OrderNotFound { id: i64 },

    #[error("material not found: {id}")]
    MaterialNotFound { id: i64 },

    #[error("user not found: {id}")]
    UserNotFound { id: i64 },

    #[error("admin not found: {login}")]
    AdminNotFound { login: String },

    #[error("invalid order status transition: {from} -> {to}")]
    InvalidStatusTransition
    {
        from: String,
        to: String,
    },

    #[error("insufficient stock for material {material_id}: requested {requested_grams}g, available {available_grams}g")]
    InsufficientStock
    {
        material_id: i64,
        requested_grams: f64,
        available_grams: f64,
    },

    #[error("a material must be defined before moving the order to status '{target}'")]
    MaterialRequiredForStatus
    {
        target: String,
    },

    #[error("validation error: {0}")]
    Validation(String),

    #[error("database error: {0}")]
    Database(String),
}
