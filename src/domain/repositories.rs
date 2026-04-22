use super::errors::DomainError;
use super::material::Material;
use super::notifications::OrderNotification;
use super::order::{NewOrder, Order};
use super::order_file::{NewOrderFile, OrderFile, StorageStats};
use super::user::{AdminUser, CasUser, User};

pub trait OrderRepository: Send + Sync
{
    fn find_all(&self) -> Result<Vec<Order>, DomainError>;
    fn find_by_user(&self, user_id: i64) -> Result<Vec<Order>, DomainError>;
    fn find_by_id(&self, id: i64) -> Result<Option<Order>, DomainError>;
    fn create(&self, order: NewOrder) -> Result<Order, DomainError>;
    fn update(&self, order: &Order) -> Result<(), DomainError>;
    fn delete(&self, id: i64) -> Result<(), DomainError>;

    fn sum_weight_by_material(
        &self,
        material_id: i64,
        exclude_order_id: Option<i64>,
    ) -> Result<f64, DomainError>;

    fn update_if_stock_sufficient(
        &self,
        order: &Order,
        spool_weight_grams: f64,
    ) -> Result<(), DomainError>;
}

pub trait OrderFileRepository: Send + Sync
{
    fn create(&self, file: NewOrderFile) -> Result<OrderFile, DomainError>;
    fn find_by_id(&self, id: i64) -> Result<Option<OrderFile>, DomainError>;
    fn find_by_order(&self, order_id: i64) -> Result<Vec<OrderFile>, DomainError>;
    fn count_by_order(&self, order_id: i64) -> Result<i64, DomainError>;
    fn delete(&self, id: i64) -> Result<(), DomainError>;
    fn storage_stats(&self) -> Result<StorageStats, DomainError>;
}

pub trait UserRepository: Send + Sync
{
    fn find_by_id(&self, id: i64) -> Result<Option<User>, DomainError>;
    fn find_by_ids(&self, ids: &[i64]) -> Result<Vec<User>, DomainError>;
    fn find_by_cas_login(&self, login: &str) -> Result<Option<User>, DomainError>;
    fn upsert_from_cas(&self, cas_user: &CasUser) -> Result<User, DomainError>;
    fn update_phone(&self, user_id: i64, phone: &str) -> Result<(), DomainError>;
}

pub trait MaterialRepository: Send + Sync
{
    fn find_all(&self) -> Result<Vec<Material>, DomainError>;
    fn find_available(&self) -> Result<Vec<Material>, DomainError>;
    fn find_by_id(&self, id: i64) -> Result<Option<Material>, DomainError>;
    fn find_by_ids(&self, ids: &[i64]) -> Result<Vec<Material>, DomainError>;
    fn upsert(&self, material: &Material) -> Result<(), DomainError>;
    fn max_id(&self) -> Result<i64, DomainError>;
    fn count_orders_using(&self, id: i64) -> Result<i64, DomainError>;
    fn delete(&self, id: i64) -> Result<(), DomainError>;
}

pub trait AdminRepository: Send + Sync
{
    fn find_by_login(&self, login: &str) -> Result<Option<AdminUser>, DomainError>;
    fn create(&self, login: &str, password_hash: &str) -> Result<AdminUser, DomainError>;
}

pub trait PasswordVerifier: Send + Sync
{
    fn verify(&self, plain: &str, hash: &str) -> Result<bool, DomainError>;
}

pub trait NotificationSender: Send + Sync
{
    /// Sends a notification about a new order. Implementations are expected
    /// to be synchronous (may block on IO). Async dispatch is the caller's
    /// responsibility.
    fn notify_new_order(&self, notification: &OrderNotification) -> Result<(), DomainError>;
}

/// Abstraction over the bytes backing an order file. Implementations
/// must ensure final writes are atomic (temp file + rename) and must
/// refuse any path that escapes the configured root.
#[async_trait::async_trait]
pub trait FileStorage: Send + Sync
{
    /// Delete a stored file by its canonical name. Missing files must
    /// be treated as success to keep DB and disk eventually consistent.
    async fn delete(&self, stored_filename: &str) -> Result<(), DomainError>;
}
