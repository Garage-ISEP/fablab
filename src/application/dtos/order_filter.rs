use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::application::dtos::order_output::OrderView;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrderFilter
{
    pub status: Option<String>,
    pub payment: Option<PaymentFilter>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentFilter
{
    Gratuit,
    Requires,
}

impl PaymentFilter
{
    pub fn as_str(&self) -> &'static str
    {
        match self
        {
            PaymentFilter::Gratuit => "gratuit",
            PaymentFilter::Requires => "requires",
        }
    }
}

impl FromStr for PaymentFilter
{
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err>
    {
        match s
        {
            "gratuit" => Ok(PaymentFilter::Gratuit),
            "requires" => Ok(PaymentFilter::Requires),
            _ => Err(()),
        }
    }
}

impl OrderFilter
{
    /// Returns true if the order matches all active filters.
    pub fn matches(&self, order: &OrderView) -> bool
    {
        if let Some(ref status) = self.status
            && status != "all"
            && &order.status != status
        {
            return false;
        }

        if let Some(payment) = self.payment
        {
            let order_pays = order.requires_payment;
            match payment
            {
                PaymentFilter::Gratuit if order_pays => return false,
                PaymentFilter::Requires if !order_pays => return false,
                _ => {}
            }
        }

        if let Some(ref q) = self.search
        {
            let needle = q.to_lowercase();
            if !needle.is_empty()
            {
                let matches_name = order.user_display_name
                    .to_lowercase()
                    .contains(&needle);
                let matches_id = order.id.to_string().contains(&needle);
                if !matches_name && !matches_id
                {
                    return false;
                }
            }
        }

        true
    }
}