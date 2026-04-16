use crate::domain::errors::DomainError;
use crate::domain::repositories::UserRepository;
use crate::domain::user::{CasUser, User};

use super::pool::DbPool;

pub struct SqliteUserRepository
{
    pool: DbPool,
}

impl SqliteUserRepository
{
    pub fn new(pool: DbPool) -> Self
    {
        Self { pool }
    }
}

fn row_to_user(row: &rusqlite::Row<'_>) -> rusqlite::Result<User>
{
    Ok(User
    {
        id: row.get(0)?,
        cas_login: row.get(1)?,
        display_name: row.get(2)?,
        email: row.get(3)?,
        phone: row.get(4)?,
        promo: row.get(5)?,
        created_at: row.get(6)?,
    })
}

const SELECT_COLS: &str = "id, cas_login, display_name, email, phone, promo, created_at";

impl UserRepository for SqliteUserRepository
{
    fn find_by_id(&self, id: i64) -> Result<Option<User>, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            let mut stmt = conn
                .prepare(&format!("SELECT {SELECT_COLS} FROM users WHERE id = ?1"))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            match stmt.query_row([id], row_to_user)
            {
                Ok(user) => Ok(Some(user)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(DomainError::Database(e.to_string())),
            }
        })
    }

    fn find_by_ids(&self, ids: &[i64]) -> Result<Vec<User>, DomainError>
    {
        if ids.is_empty()
        {
            return Ok(Vec::new());
        }

        self.pool.with_conn(|conn|
        {
            let mut stmt = conn
                .prepare(&format!("SELECT {SELECT_COLS} FROM users WHERE id = ?1"))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            let mut results = Vec::with_capacity(ids.len());
            for id in ids
            {
                match stmt.query_row([id], row_to_user)
                {
                    Ok(user) => results.push(user),
                    Err(rusqlite::Error::QueryReturnedNoRows) => {}
                    Err(e) => return Err(DomainError::Database(e.to_string())),
                }
            }
            Ok(results)
        })
    }

    fn find_by_cas_login(&self, login: &str) -> Result<Option<User>, DomainError>
    {
        let login = login.to_owned();
        self.pool.with_conn(|conn|
        {
            let mut stmt = conn
                .prepare(&format!("SELECT {SELECT_COLS} FROM users WHERE cas_login = ?1"))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            match stmt.query_row([&login], row_to_user)
            {
                Ok(user) => Ok(Some(user)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(DomainError::Database(e.to_string())),
            }
        })
    }

    fn upsert_from_cas(&self, cas_user: &CasUser) -> Result<User, DomainError>
    {
        let cas_user = cas_user.clone();
        self.pool.with_conn(|conn|
        {
            conn.execute(
                "INSERT INTO users (cas_login, display_name, email, promo) \
                 VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(cas_login) DO UPDATE SET \
                     display_name = excluded.display_name, \
                     email = excluded.email, \
                     promo = excluded.promo",
                rusqlite::params![
                    cas_user.cas_login,
                    cas_user.display_name,
                    cas_user.email,
                    cas_user.promo,
                ],
            )
            .map_err(|e| DomainError::Database(e.to_string()))?;

            let mut stmt = conn
                .prepare(&format!("SELECT {SELECT_COLS} FROM users WHERE cas_login = ?1"))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            stmt.query_row([&cas_user.cas_login], row_to_user)
                .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn update_phone(&self, user_id: i64, phone: &str) -> Result<(), DomainError>
    {
        let phone = phone.to_owned();
        self.pool.with_conn(|conn|
        {
            let rows = conn
                .execute(
                    "UPDATE users SET phone = ?1 WHERE id = ?2",
                    rusqlite::params![phone, user_id],
                )
                .map_err(|e| DomainError::Database(e.to_string()))?;

            if rows == 0
            {
                return Err(DomainError::UserNotFound { id: user_id });
            }
            Ok(())
        })
    }
}
