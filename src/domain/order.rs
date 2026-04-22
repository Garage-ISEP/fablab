use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::errors::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus
{
    ATraiter,
    EnTraitement,
    Imprime,
    Livre,
    Annule,
}

impl OrderStatus
{
    pub fn as_str(&self) -> &'static str
    {
        match self
        {
            OrderStatus::ATraiter => "a_traiter",
            OrderStatus::EnTraitement => "en_traitement",
            OrderStatus::Imprime => "imprime",
            OrderStatus::Livre => "livre",
            OrderStatus::Annule => "annule",
        }
    }

    pub fn can_transition_to(&self, target: OrderStatus) -> bool
    {
        matches!
        (
            (self, target),
            (OrderStatus::ATraiter, OrderStatus::EnTraitement)
                | (OrderStatus::ATraiter, OrderStatus::Annule)
                | (OrderStatus::EnTraitement, OrderStatus::Imprime)
                | (OrderStatus::EnTraitement, OrderStatus::Annule)
                | (OrderStatus::Imprime, OrderStatus::Livre)
                | (OrderStatus::Imprime, OrderStatus::Annule)
        )
    }

    pub fn transition_to(&self, target: OrderStatus) -> Result<OrderStatus, DomainError>
    {
        if *self == target
        {
            return Ok(target);
        }

        if self.can_transition_to(target)
        {
            Ok(target)
        }
        else
        {
            Err(DomainError::InvalidStatusTransition
            {
                from: self.as_str().to_owned(),
                to: target.as_str().to_owned(),
            })
        }
    }
}

impl FromStr for OrderStatus
{
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err>
    {
        match s
        {
            "a_traiter" => Ok(OrderStatus::ATraiter),
            "en_traitement" => Ok(OrderStatus::EnTraitement),
            "imprime" => Ok(OrderStatus::Imprime),
            "livre" => Ok(OrderStatus::Livre),
            "annule" => Ok(OrderStatus::Annule),
            other => Err(DomainError::Validation
            (
                format!("unknown order status: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Order
{
    pub id: i64,
    pub user_id: i64,
    pub created_at: String,
    pub software_used: String,
    pub material_id: Option<i64>,
    pub quantity: i32,
    pub comments: Option<String>,
    pub status: OrderStatus,
    pub requires_payment: bool,
    pub sliced_weight_grams: Option<f64>,
    pub print_time_minutes: Option<i32>,
}

impl Order
{
    pub fn try_advance_status(&self, target: OrderStatus) -> Result<OrderStatus, DomainError>
    {
        if self.material_id.is_none()
            && matches!(
                target,
                OrderStatus::EnTraitement | OrderStatus::Imprime | OrderStatus::Livre
            )
        {
            return Err(DomainError::MaterialRequiredForStatus
            {
                target: target.as_str().to_owned(),
            });
        }

        self.status.transition_to(target)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewOrder
{
    pub user_id: i64,
    pub software_used: String,
    pub material_id: Option<i64>,
    pub quantity: i32,
    pub comments: Option<String>,
}