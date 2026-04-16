use std::collections::HashMap;

use async_trait::async_trait;
use tower_sessions::session::{Id, Record};
use tower_sessions::session_store;
use tower_sessions::SessionStore;

use super::pool::DbPool;
use crate::domain::errors::DomainError;

/// SQLite-backed session store. Sessions survive server restarts.
#[derive(Clone, Debug)]
pub struct SqliteSessionStore
{
    pool: DbPool,
}

impl SqliteSessionStore
{
    pub fn new(pool: DbPool) -> Result<Self, DomainError>
    {
        pool.with_conn(|conn|
        {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS sessions \
                 (\
                     id TEXT PRIMARY KEY, \
                     data TEXT NOT NULL, \
                     expiry_date TEXT NOT NULL\
                 );"
            )
            .map_err(|e| DomainError::Database(format!("session table creation: {e}")))?;
            Ok(())
        })?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl SessionStore for SqliteSessionStore
{
    async fn save(&self, record: &Record) -> session_store::Result<()>
    {
        let id = record.id.to_string();
        let expiry = record.expiry_date
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();

        let data = serde_json::to_string(&record.data)
            .map_err(|e| session_store::Error::Encode(e.to_string()))?;

        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move ||
        {
            pool.with_conn(|conn|
            {
                conn.execute(
                    "INSERT INTO sessions (id, data, expiry_date) \
                     VALUES (?1, ?2, ?3) \
                     ON CONFLICT(id) DO UPDATE SET \
                         data = excluded.data, \
                         expiry_date = excluded.expiry_date",
                    rusqlite::params![id, data, expiry],
                )
                .map_err(|e| DomainError::Database(e.to_string()))?;
                Ok(())
            })
        })
        .await
        .map_err(|e| session_store::Error::Backend(e.to_string()))?
        .map_err(|e: DomainError| session_store::Error::Backend(e.to_string()))
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>>
    {
        let id = session_id.to_string();
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move ||
        {
            pool.with_conn(|conn|
            {
                let mut stmt = conn
                    .prepare("SELECT data, expiry_date FROM sessions WHERE id = ?1")
                    .map_err(|e| DomainError::Database(e.to_string()))?;

                let result = stmt.query_row([&id], |row|
                {
                    let data_str: String = row.get(0)?;
                    let expiry_str: String = row.get(1)?;
                    Ok((data_str, expiry_str))
                });

                match result
                {
                    Ok((data_str, expiry_str)) =>
                    {
                        let expiry = time::OffsetDateTime::parse(
                            &expiry_str,
                            &time::format_description::well_known::Rfc3339,
                        )
                        .map_err(|e| DomainError::Validation(format!("session expiry parse: {e}")))?;

                        if expiry <= time::OffsetDateTime::now_utc()
                        {
                            let _ = conn.execute("DELETE FROM sessions WHERE id = ?1", [&id]);
                            return Ok(None);
                        }

                        let data: HashMap<String, serde_json::Value> =
                            serde_json::from_str(&data_str)
                                .map_err(|e| DomainError::Validation(format!("session data parse: {e}")))?;

                        let session_id: Id = id
                            .parse()
                            .map_err(|e| DomainError::Validation(format!("session id parse: {e}")))?;

                        Ok(Some(Record { id: session_id, data, expiry_date: expiry }))
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(DomainError::Database(e.to_string())),
                }
            })
        })
        .await
        .map_err(|e| session_store::Error::Backend(e.to_string()))?
        .map_err(|e: DomainError| session_store::Error::Backend(e.to_string()))
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()>
    {
        let id = session_id.to_string();
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move ||
        {
            pool.with_conn(|conn|
            {
                conn.execute("DELETE FROM sessions WHERE id = ?1", [&id])
                    .map_err(|e| DomainError::Database(e.to_string()))?;
                Ok(())
            })
        })
        .await
        .map_err(|e| session_store::Error::Backend(e.to_string()))?
        .map_err(|e: DomainError| session_store::Error::Backend(e.to_string()))
    }
}
