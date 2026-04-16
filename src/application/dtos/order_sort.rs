use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::application::dtos::order_output::OrderView;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortColumn
{
    Id,
    CreatedAt,
    Client,
    Material,
    Quantity,
    Status,
    RequiresPayment,
    Weight,
    PrintTime,
}

impl SortColumn
{
    pub fn as_str(&self) -> &'static str
    {
        match self
        {
            SortColumn::Id => "id",
            SortColumn::CreatedAt => "date",
            SortColumn::Client => "client",
            SortColumn::Material => "material",
            SortColumn::Quantity => "quantity",
            SortColumn::Status => "status",
            SortColumn::RequiresPayment => "payment",
            SortColumn::Weight => "weight",
            SortColumn::PrintTime => "time",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection
{
    Asc,
    Desc,
}

impl SortDirection
{
    pub fn toggled(&self) -> Self
    {
        match self
        {
            SortDirection::Asc => SortDirection::Desc,
            SortDirection::Desc => SortDirection::Asc,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderSort
{
    pub column: SortColumn,
    pub direction: SortDirection,
}

impl OrderSort
{
    pub fn new(column: SortColumn, direction: SortDirection) -> Self
    {
        Self { column, direction }
    }

    /// Default sort: most recent first (by id desc, since ids are monotonic).
    pub fn default_recent() -> Self
    {
        Self
        {
            column: SortColumn::Id,
            direction: SortDirection::Desc,
        }
    }

    /// Compares two orders according to the column, then applies direction.
    pub fn compare(&self, a: &OrderView, b: &OrderView) -> Ordering
    {
        let raw = match self.column
        {
            SortColumn::Id => a.id.cmp(&b.id),
            SortColumn::CreatedAt => a.created_at.cmp(&b.created_at),
            SortColumn::Client => a.user_display_name
                .to_lowercase()
                .cmp(&b.user_display_name.to_lowercase()),
            SortColumn::Material => option_str_cmp(
                a.material_label.as_deref(),
                b.material_label.as_deref(),
            ),
            SortColumn::Quantity => a.quantity.cmp(&b.quantity),
            SortColumn::Status => a.status.cmp(&b.status),
            SortColumn::RequiresPayment => a.requires_payment.cmp(&b.requires_payment),
            SortColumn::Weight => option_f64_cmp(
                a.sliced_weight_grams,
                b.sliced_weight_grams,
            ),
            SortColumn::PrintTime => option_ord_cmp(
                a.print_time_minutes,
                b.print_time_minutes,
            ),
        };

        match self.direction
        {
            SortDirection::Asc => raw,
            SortDirection::Desc => raw.reverse(),
        }
    }
}

/// None values sort after Some values in ascending order.
fn option_str_cmp(a: Option<&str>, b: Option<&str>) -> Ordering
{
    match (a, b)
    {
        (Some(x), Some(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// None values sort after Some values in ascending order.
/// NaN is treated as equal to itself for stable ordering.
fn option_f64_cmp(a: Option<f64>, b: Option<f64>) -> Ordering
{
    match (a, b)
    {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// None values sort after Some values in ascending order.
fn option_ord_cmp<T: Ord>(a: Option<T>, b: Option<T>) -> Ordering
{
    match (a, b)
    {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}