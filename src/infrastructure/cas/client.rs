use crate::domain::errors::DomainError;
use crate::domain::user::CasUser;

use super::parser::parse_cas_response;

pub struct CasClient
{
    base_url: String,
    http: reqwest::blocking::Client,
}

impl CasClient
{
    pub fn new(base_url: String) -> Self
    {
        let http = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self
        {
            base_url,
            http,
        }
    }

    pub fn base_url(&self) -> &str
    {
        &self.base_url
    }

    pub fn validate_ticket(
        &self,
        ticket: &str,
        service_url: &str,
    ) -> Result<CasUser, DomainError>
    {
        let base = format!("{}/serviceValidate", self.base_url);
        let mut url = reqwest::Url::parse(&base)
            .map_err(|e| DomainError::Validation(format!("invalid CAS base URL: {e}")))?;
        url.query_pairs_mut()
            .append_pair("service", service_url)
            .append_pair("ticket", ticket);

        let response = self.http
            .get(url)
            .send()
            .map_err(|e| DomainError::Validation(format!("CAS HTTP request failed: {e}")))?;

        let body = response
            .text()
            .map_err(|e| DomainError::Validation(format!("CAS response read error: {e}")))?;

        parse_cas_response(&body)
    }
}
