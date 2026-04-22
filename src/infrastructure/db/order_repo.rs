use std::str::FromStr;

use crate::domain::errors::DomainError;
use crate::domain::order::{NewOrder, Order, OrderStatus};
use crate::domain::repositories::OrderRepository;

use super::pool::DbPool;

pub struct SqliteOrderRepository
{
    pool: DbPool,
}

impl SqliteOrderRepository
{
    pub fn new(pool: DbPool) -> Self
    {
        Self { pool }
    }
}

fn row_to_order(row: &rusqlite::Row<'_>) -> rusqlite::Result<Order>
{
    let status_str: String = row.get(7)?;
    let req_int: i64 = row.get(8)?;

    let status = OrderStatus::from_str(&status_str).unwrap_or(OrderStatus::ATraiter);

    Ok(Order
    {
        id: row.get(0)?,
        user_id: row.get(1)?,
        created_at: row.get(2)?,
        software_used: row.get(3)?,
        material_id: row.get(4)?,
        quantity: row.get(5)?,
        comments: row.get(6)?,
        status,
        requires_payment: req_int != 0,
        sliced_weight_grams: row.get(9)?,
        print_time_minutes: row.get(10)?,
    })
}

const SELECT_COLS: &str =
    "id, user_id, created_at, software_used, material_id, \
     quantity, comments, status, requires_payment, \
     sliced_weight_grams, print_time_minutes";

impl OrderRepository for SqliteOrderRepository
{
    fn find_all(&self) -> Result<Vec<Order>, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            let mut stmt = conn
                .prepare(&format!("SELECT {SELECT_COLS} FROM orders ORDER BY created_at DESC"))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            stmt.query_map([], row_to_order)
                .map_err(|e| DomainError::Database(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn find_by_user(&self, user_id: i64) -> Result<Vec<Order>, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT {SELECT_COLS} FROM orders WHERE user_id = ?1 ORDER BY created_at DESC"
                ))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            stmt.query_map([user_id], row_to_order)
                .map_err(|e| DomainError::Database(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn find_by_id(&self, id: i64) -> Result<Option<Order>, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            let mut stmt = conn
                .prepare(&format!("SELECT {SELECT_COLS} FROM orders WHERE id = ?1"))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            match stmt.query_row([id], row_to_order)
            {
                Ok(order) => Ok(Some(order)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(DomainError::Database(e.to_string())),
            }
        })
    }

    fn create(&self, order: NewOrder) -> Result<Order, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            conn.execute(
                "INSERT INTO orders (user_id, software_used, material_id, \
                 quantity, comments) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    order.user_id, order.software_used,
                    order.material_id, order.quantity, order.comments,
                ],
            )
            .map_err(|e| DomainError::Database(e.to_string()))?;

            let id = conn.last_insert_rowid();
            let mut stmt = conn
                .prepare(&format!("SELECT {SELECT_COLS} FROM orders WHERE id = ?1"))
                .map_err(|e| DomainError::Database(e.to_string()))?;

            stmt.query_row([id], row_to_order)
                .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn update(&self, order: &Order) -> Result<(), DomainError>
    {
        self.pool.with_conn(|conn|
        {
            let rows = conn
                .execute(
                    "UPDATE orders SET \
                     status = ?1, requires_payment = ?2, sliced_weight_grams = ?3, \
                     print_time_minutes = ?4, material_id = ?5 \
                     WHERE id = ?6",
                    rusqlite::params![
                        order.status.as_str(),
                        i64::from(order.requires_payment),
                        order.sliced_weight_grams,
                        order.print_time_minutes,
                        order.material_id,
                        order.id,
                    ],
                )
                .map_err(|e| DomainError::Database(e.to_string()))?;

            if rows == 0
            {
                return Err(DomainError::OrderNotFound { id: order.id });
            }
            Ok(())
        })
    }

    fn delete(&self, id: i64) -> Result<(), DomainError>
    {
        self.pool.with_conn(|conn|
        {
            let rows = conn.execute("DELETE FROM orders WHERE id = ?1", [id])
                .map_err(|e| DomainError::Database(e.to_string()))?;
            if rows == 0
            {
                return Err(DomainError::OrderNotFound { id });
            }
            Ok(())
        })
    }

    fn sum_weight_by_material(
        &self,
        material_id: i64,
        exclude_order_id: Option<i64>,
    ) -> Result<f64, DomainError>
    {
        self.pool.with_conn(|conn|
        {
            conn.query_row(
                "SELECT COALESCE(SUM(sliced_weight_grams), 0.0) \
                 FROM orders \
                 WHERE material_id = ?1 \
                   AND status != 'annule' \
                   AND (?2 IS NULL OR id != ?2)",
                rusqlite::params![material_id, exclude_order_id],
                |row| row.get::<_, f64>(0),
            )
            .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn update_if_stock_sufficient(
        &self,
        order: &Order,
        spool_weight_grams: f64,
    ) -> Result<(), DomainError>
    {
        let material_id = order.material_id.ok_or_else(||
            DomainError::Validation(
                "update_if_stock_sufficient requires a material_id on the order".to_owned(),
            )
        )?;
        let requested = order.sliced_weight_grams.ok_or_else(||
            DomainError::Validation(
                "update_if_stock_sufficient requires sliced_weight_grams on the order".to_owned(),
            )
        )?;

        self.pool.with_transaction(|tx|
        {
            let consumed: f64 = tx.query_row(
                "SELECT COALESCE(SUM(sliced_weight_grams), 0.0) \
                 FROM orders \
                 WHERE material_id = ?1 \
                   AND status != 'annule' \
                   AND id != ?2",
                rusqlite::params![material_id, order.id],
                |row| row.get::<_, f64>(0),
            )
            .map_err(|e| DomainError::Database(e.to_string()))?;

            crate::domain::stock::check_sufficient(
                material_id,
                spool_weight_grams,
                consumed,
                requested,
            )?;

            let rows = tx.execute(
                "UPDATE orders SET \
                 status = ?1, requires_payment = ?2, sliced_weight_grams = ?3, \
                 print_time_minutes = ?4, material_id = ?5 \
                 WHERE id = ?6",
                rusqlite::params![
                    order.status.as_str(),
                    i64::from(order.requires_payment),
                    order.sliced_weight_grams,
                    order.print_time_minutes,
                    order.material_id,
                    order.id,
                ],
            )
            .map_err(|e| DomainError::Database(e.to_string()))?;

            if rows == 0
            {
                return Err(DomainError::OrderNotFound { id: order.id });
            }
            Ok(())
        })
    }
}
