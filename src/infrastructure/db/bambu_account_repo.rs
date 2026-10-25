use crate::domain::errors::DomainError;
use crate::domain::filament::{BambuAccount, BambuCredentials, PrinterDevice};
use crate::domain::repositories::BambuAccountRepository;

use super::pool::DbPool;

/// Single-row table: saving a new token replaces the previous one.
pub struct SqliteBambuAccountRepository {
    pool: DbPool,
}

impl SqliteBambuAccountRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl BambuAccountRepository for SqliteBambuAccountRepository {
    fn load(&self) -> Result<Option<BambuAccount>, DomainError> {
        self.pool.with_conn(|conn| {
            let row = conn.query_row(
                "SELECT user_id, access_token, devices_json, updated_at \
                 FROM bambu_account WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            );

            let (user_id, access_token, devices_json, updated_at) = match row {
                Ok(values) => values,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(e) => return Err(DomainError::Database(e.to_string())),
            };

            let devices: Vec<PrinterDevice> = serde_json::from_str(&devices_json)
                .map_err(|e| DomainError::Database(format!("bambu devices: {e}")))?;

            Ok(Some(BambuAccount {
                credentials: BambuCredentials {
                    user_id,
                    access_token,
                },
                devices,
                updated_at,
            }))
        })
    }

    fn save(
        &self,
        credentials: &BambuCredentials,
        devices: &[PrinterDevice],
    ) -> Result<(), DomainError> {
        let devices_json = serde_json::to_string(devices)
            .map_err(|e| DomainError::Database(format!("bambu devices: {e}")))?;

        self.pool.with_conn(|conn| {
            conn.execute(
                "INSERT INTO bambu_account (id, user_id, access_token, devices_json, updated_at) \
                 VALUES (1, ?1, ?2, ?3, datetime('now')) \
                 ON CONFLICT(id) DO UPDATE SET \
                     user_id = excluded.user_id, \
                     access_token = excluded.access_token, \
                     devices_json = excluded.devices_json, \
                     updated_at = excluded.updated_at",
                rusqlite::params![credentials.user_id, credentials.access_token, devices_json],
            )
            .map_err(|e| DomainError::Database(e.to_string()))?;
            Ok(())
        })
    }
}
