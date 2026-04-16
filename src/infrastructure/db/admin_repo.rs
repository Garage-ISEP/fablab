use crate::domain::errors::DomainError;
use crate::domain::repositories::AdminRepository;
use crate::domain::user::AdminUser;

use super::pool::DbPool;

pub struct SqliteAdminRepository
{
    pool: DbPool,
}

impl SqliteAdminRepository
{
    pub fn new(pool: DbPool) -> Self
    {
        Self { pool }
    }
}

impl AdminRepository for SqliteAdminRepository
{
    fn find_by_login(&self, login: &str) -> Result<Option<AdminUser>, DomainError>
    {
        let login = login.to_owned();
        self.pool.with_conn(|conn|
        {
            let mut stmt = conn
                .prepare("SELECT id, login, password_hash FROM admin_users WHERE login = ?1")
                .map_err(|e| DomainError::Database(e.to_string()))?;

            let result = stmt.query_row([&login], |row|
            {
                Ok(AdminUser
                {
                    id: row.get(0)?,
                    login: row.get(1)?,
                    password_hash: row.get(2)?,
                })
            });

            match result
            {
                Ok(admin) => Ok(Some(admin)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(DomainError::Database(e.to_string())),
            }
        })
    }

    fn create(&self, login: &str, password_hash: &str) -> Result<AdminUser, DomainError>
    {
        let login = login.to_owned();
        let password_hash = password_hash.to_owned();
        self.pool.with_conn(|conn|
        {
            conn.execute(
                "INSERT INTO admin_users (login, password_hash) VALUES (?1, ?2)",
                rusqlite::params![login, password_hash],
            )
            .map_err(|e| DomainError::Database(e.to_string()))?;

            let id = conn.last_insert_rowid();
            Ok(AdminUser { id, login, password_hash })
        })
    }
}
