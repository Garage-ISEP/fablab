use crate::domain::errors::DomainError;
use crate::domain::order_file::{NewOrderFile, OrderFile, StorageStats};
use crate::domain::repositories::OrderFileRepository;

use super::pool::DbPool;

pub struct SqliteOrderFileRepository
{
    pool: DbPool,
}

impl SqliteOrderFileRepository
{
    pub fn new(pool: DbPool) -> Self
    {
        Self { pool }
    }
}

fn row_to_order_file(row: &rusqlite::Row<'_>) -> rusqlite::Result<OrderFile>
{
    Ok(OrderFile
    {
        id: row.get(0)?,
        order_id: row.get(1)?,
        original_filename: row.get(2)?,
        stored_filename: row.get(3)?,
        size_bytes: row.get(4)?,
        mime_type: row.get(5)?,
        uploaded_at: row.get(6)?,
    })
}

const SELECT_COLS: &str =
    "id, order_id, original_filename, stored_filename, size_bytes, mime_type, uploaded_at";

impl OrderFileRepository for SqliteOrderFileRepository
{
    fn create(&self, file: NewOrderFile) -> Result<OrderFile, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            conn.execute(
                "INSERT INTO order_files \
                 (order_id, original_filename, stored_filename, size_bytes, mime_type) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    file.order_id,
                    file.original_filename,
                    file.stored_filename,
                    file.size_bytes,
                    file.mime_type,
                ],
            )
            .map_err(|e| DomainError::Database(e.to_string()))?;

            let id = conn.last_insert_rowid();
            let mut stmt = conn
                .prepare(&format!("SELECT {SELECT_COLS} FROM order_files WHERE id = ?1"))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            stmt.query_row([id], row_to_order_file)
                .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn find_by_id(&self, id: i64) -> Result<Option<OrderFile>, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            let mut stmt = conn
                .prepare(&format!("SELECT {SELECT_COLS} FROM order_files WHERE id = ?1"))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            match stmt.query_row([id], row_to_order_file)
            {
                Ok(f) => Ok(Some(f)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(DomainError::Database(e.to_string())),
            }
        })
    }

    fn find_by_order(&self, order_id: i64) -> Result<Vec<OrderFile>, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT {SELECT_COLS} FROM order_files \
                     WHERE order_id = ?1 ORDER BY id ASC"
                ))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            stmt.query_map([order_id], row_to_order_file)
                .map_err(|e| DomainError::Database(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn count_by_order(&self, order_id: i64) -> Result<i64, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            conn.query_row(
                "SELECT COUNT(*) FROM order_files WHERE order_id = ?1",
                [order_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn delete(&self, id: i64) -> Result<(), DomainError>
    {
        self.pool.with_conn(|conn|
        {
            let rows = conn.execute("DELETE FROM order_files WHERE id = ?1", [id])
                .map_err(|e| DomainError::Database(e.to_string()))?;
            if rows == 0
            {
                return Err(DomainError::Database(format!("file {id} not found")));
            }
            Ok(())
        })
    }

    fn storage_stats(&self) -> Result<StorageStats, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            conn.query_row(
                "SELECT COUNT(*), COALESCE(SUM(size_bytes), 0) FROM order_files",
                [],
                |row| Ok(StorageStats
                {
                    total_files: row.get(0)?,
                    total_bytes: row.get(1)?,
                }),
            )
            .map_err(|e| DomainError::Database(e.to_string()))
        })
    }
}
