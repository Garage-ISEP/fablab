use serde::{Deserialize, Serialize};

/// A file attached to an order, stored on the local filesystem and
/// referenced by its stored filename (a UUID + canonical extension).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderFile
{
    pub id: i64,
    pub order_id: i64,
    pub original_filename: String,
    pub stored_filename: String,
    pub size_bytes: i64,
    pub mime_type: String,
    pub uploaded_at: String,
}

/// Payload used to persist a new order file after bytes have been
/// written to disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewOrderFile
{
    pub order_id: i64,
    pub original_filename: String,
    pub stored_filename: String,
    pub size_bytes: i64,
    pub mime_type: String,
}

/// Aggregate storage statistics for the admin dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StorageStats
{
    pub total_files: i64,
    pub total_bytes: i64,
}
