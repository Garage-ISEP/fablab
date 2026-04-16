use crate::domain::errors::DomainError;

use super::pool::DbPool;

const MIGRATIONS: &[(&str, &str)] = &[
    ("001_create_db", include_str!("../../../migrations/001_create_db.sql")),
];

pub fn run_migrations(pool: &DbPool) -> Result<(), DomainError>
{
    pool.with_conn(|conn|
    {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _migrations \
             (name TEXT PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));"
        )
        .map_err(|e| DomainError::Database(e.to_string()))?;

        for (name, sql) in MIGRATIONS
        {
            let already_applied: bool = conn
                .prepare("SELECT COUNT(*) FROM _migrations WHERE name = ?1")
                .and_then(|mut stmt| stmt.query_row([name], |row| row.get::<_, i64>(0)))
                .map(|count| count > 0)
                .map_err(|e| DomainError::Database(e.to_string()))?;

            if !already_applied
            {
                conn.execute_batch(sql)
                    .map_err(|e| DomainError::Database(format!("migration {name} failed: {e}")))?;
                conn.execute("INSERT INTO _migrations (name) VALUES (?1)", [name])
                    .map_err(|e| DomainError::Database(e.to_string()))?;
            }
        }

        Ok(())
    })
}
