use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;

use crate::domain::errors::DomainError;

/// Thread-safe connection pool around SQLite (WAL mode).
#[derive(Clone, Debug)]
pub struct DbPool
{
    pool: Pool<SqliteConnectionManager>,
}

impl DbPool
{
    pub fn open(path: &str) -> Result<Self, DomainError>
    {
        let manager = SqliteConnectionManager::file(path)
            .with_init(|conn|
            {
                conn.execute_batch(
                    "PRAGMA journal_mode=WAL;\
                     PRAGMA foreign_keys=ON;\
                     PRAGMA busy_timeout=5000;"
                )?;
                Ok(())
            });

        let pool = Pool::builder()
            .max_size(8)
            .build(manager)
            .map_err(|e| DomainError::Database(format!("pool build error: {e}")))?;

        Ok(Self { pool })
    }

    pub fn open_in_memory() -> Result<Self, DomainError>
    {
        let manager = SqliteConnectionManager::memory()
            .with_init(|conn|
            {
                conn.execute_batch("PRAGMA foreign_keys=ON;")?;
                Ok(())
            });

        let pool = Pool::builder()
            .max_size(1)
            .build(manager)
            .map_err(|e| DomainError::Database(format!("pool build error: {e}")))?;

        Ok(Self { pool })
    }

    pub fn with_conn<F, T>(&self, f: F) -> Result<T, DomainError>
    where
        F: FnOnce(&Connection) -> Result<T, DomainError>,
    {
        let conn: PooledConnection<SqliteConnectionManager> = self.pool
            .get()
            .map_err(|e| DomainError::Database(format!("pool get error: {e}")))?;
        f(&conn)
    }
}
