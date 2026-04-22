use crate::domain::errors::DomainError;
use crate::domain::material::Material;
use crate::domain::repositories::MaterialRepository;

use super::pool::DbPool;

pub struct SqliteMaterialRepository
{
    pool: DbPool,
}

impl SqliteMaterialRepository
{
    pub fn new(pool: DbPool) -> Self
    {
        Self { pool }
    }
}

fn row_to_material(row: &rusqlite::Row<'_>) -> rusqlite::Result<Material>
{
    let available_int: i64 = row.get(3)?;
    Ok(Material
    {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        available: available_int != 0,
        spool_weight_grams: row.get(4)?,
    })
}

const SELECT_COLS: &str = "id, name, color, available, spool_weight_grams";

impl MaterialRepository for SqliteMaterialRepository
{
    fn find_all(&self) -> Result<Vec<Material>, DomainError>
    {
        self.pool.with_conn(|conn|
        {
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

    fn find_available(&self) -> Result<Vec<Material>, DomainError>
    {
        self.pool.with_conn(|conn|
        {
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

    fn find_by_id(&self, id: i64) -> Result<Option<Material>, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            let mut stmt = conn
                .prepare(&format!("SELECT {SELECT_COLS} FROM materials WHERE id = ?1"))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            match stmt.query_row([id], row_to_material)
            {
                Ok(mat) => Ok(Some(mat)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(DomainError::Database(e.to_string())),
            }
        })
    }

    fn find_by_ids(&self, ids: &[i64]) -> Result<Vec<Material>, DomainError>
    {
        if ids.is_empty()
        {
            return Ok(Vec::new());
        }

        self.pool.with_conn(|conn|
        {
            let mut stmt = conn
                .prepare(&format!("SELECT {SELECT_COLS} FROM materials WHERE id = ?1"))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            let mut results = Vec::with_capacity(ids.len());
            for id in ids
            {
                match stmt.query_row([id], row_to_material)
                {
                    Ok(mat) => results.push(mat),
                    Err(rusqlite::Error::QueryReturnedNoRows) => {}
                    Err(e) => return Err(DomainError::Database(e.to_string())),
                }
            }
            Ok(results)
        })
    }

    fn upsert(&self, material: &Material) -> Result<(), DomainError>
    {
        self.pool.with_conn(|conn|
        {
            conn.execute(
                "INSERT INTO materials (id, name, color, available, spool_weight_grams) \
                 VALUES (?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT(id) DO UPDATE SET \
                     name = excluded.name, \
                     color = excluded.color, \
                     available = excluded.available, \
                     spool_weight_grams = excluded.spool_weight_grams",
                rusqlite::params![
                    material.id,
                    material.name,
                    material.color,
                    i64::from(material.available),
                    material.spool_weight_grams,
                ],
            )
            .map_err(|e| DomainError::Database(e.to_string()))?;

            Ok(())
        })
    }

    fn max_id(&self) -> Result<i64, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            conn.query_row(
                "SELECT COALESCE(MAX(id), 0) FROM materials",
                [],
                |row| row.get(0),
            )
            .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn count_orders_using(&self, id: i64) -> Result<i64, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            conn.query_row(
                "SELECT COUNT(*) FROM orders WHERE material_id = ?1",
                [id],
                |row| row.get(0),
            )
            .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn delete(&self, id: i64) -> Result<(), DomainError>
    {
        self.pool.with_conn(|conn|
        {
            let rows = conn.execute("DELETE FROM materials WHERE id = ?1", [id])
                .map_err(|e| DomainError::Database(e.to_string()))?;
            if rows == 0
            {
                return Err(DomainError::MaterialNotFound { id });
            }
            Ok(())
        })
    }
}