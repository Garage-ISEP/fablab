use crate::domain::errors::DomainError;
use crate::domain::material::{LoadedMaterial, Material};
use crate::domain::repositories::MaterialRepository;

use super::pool::DbPool;

pub struct SqliteMaterialRepository {
    pool: DbPool,
}

impl SqliteMaterialRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

fn row_to_material(row: &rusqlite::Row<'_>) -> rusqlite::Result<Material> {
    let available_int: i64 = row.get(5)?;
    Ok(Material {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        color_hex: row.get(3)?,
        bambu_key: row.get(4)?,
        available: available_int != 0,
    })
}

const SELECT_COLS: &str = "id, name, color, color_hex, bambu_key, available";

impl MaterialRepository for SqliteMaterialRepository {
    fn find_all(&self) -> Result<Vec<Material>, DomainError> {
        self.pool.with_conn(|conn| {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT {SELECT_COLS} FROM materials ORDER BY name, color"
                ))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            stmt.query_map([], row_to_material)
                .map_err(|e| DomainError::Database(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn find_available(&self) -> Result<Vec<Material>, DomainError> {
        self.pool.with_conn(|conn| {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT {SELECT_COLS} FROM materials \
                     WHERE available = 1 ORDER BY name, color"
                ))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            stmt.query_map([], row_to_material)
                .map_err(|e| DomainError::Database(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn find_by_id(&self, id: i64) -> Result<Option<Material>, DomainError> {
        self.pool.with_conn(|conn| {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT {SELECT_COLS} FROM materials WHERE id = ?1"
                ))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            match stmt.query_row([id], row_to_material) {
                Ok(mat) => Ok(Some(mat)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(DomainError::Database(e.to_string())),
            }
        })
    }

    fn find_by_ids(&self, ids: &[i64]) -> Result<Vec<Material>, DomainError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        self.pool.with_conn(|conn| {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT {SELECT_COLS} FROM materials WHERE id = ?1"
                ))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            let mut results = Vec::with_capacity(ids.len());
            for id in ids {
                match stmt.query_row([id], row_to_material) {
                    Ok(mat) => results.push(mat),
                    Err(rusqlite::Error::QueryReturnedNoRows) => {}
                    Err(e) => return Err(DomainError::Database(e.to_string())),
                }
            }
            Ok(results)
        })
    }

    fn sync_loaded(&self, loaded: &[LoadedMaterial]) -> Result<(), DomainError> {
        self.pool.with_transaction(|tx| {
            let db_err = |e: rusqlite::Error| DomainError::Database(e.to_string());

            tx.execute("UPDATE materials SET available = 0", [])
                .map_err(db_err)?;

            for material in loaded {
                // A hand-made material with the same label (from before the
                // AMS integration) becomes this product, so its orders stay
                // attached to it.
                tx.execute(
                    "UPDATE materials SET bambu_key = ?1 \
                     WHERE id = (SELECT id FROM materials \
                                 WHERE bambu_key IS NULL AND name = ?2 AND color = ?3 \
                                 ORDER BY id LIMIT 1) \
                       AND NOT EXISTS (SELECT 1 FROM materials WHERE bambu_key = ?1)",
                    rusqlite::params![material.bambu_key, material.name, material.color],
                )
                .map_err(db_err)?;

                tx.execute(
                    "INSERT INTO materials (name, color, color_hex, bambu_key, available) \
                     VALUES (?1, ?2, ?3, ?4, 1) \
                     ON CONFLICT(bambu_key) DO UPDATE SET \
                         name = excluded.name, \
                         color = excluded.color, \
                         color_hex = excluded.color_hex, \
                         available = 1",
                    rusqlite::params![
                        material.name,
                        material.color,
                        material.color_hex,
                        material.bambu_key,
                    ],
                )
                .map_err(db_err)?;
            }

            Ok(())
        })
    }
}
