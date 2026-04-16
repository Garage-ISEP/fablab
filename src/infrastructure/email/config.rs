use std::env;

#[derive(Debug, Clone)]
pub struct EmailConfig
{
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_password: String,
    pub from_address: String,
    pub admin_address: String,
}

impl EmailConfig
{
    /// Loads the email configuration from environment variables.
    /// Returns a list of missing variable names if any are absent.
    pub fn from_env() -> Result<Self, Vec<String>>
    {
        let mut missing: Vec<String> = Vec::new();

        let smtp_host = env::var("SMTP_HOST")
            .unwrap_or_else(|_|
            {
                missing.push("SMTP_HOST".to_owned());
                String::new()
            });
        let smtp_port_raw = env::var("SMTP_PORT")
            .unwrap_or_else(|_|
            {
                missing.push("SMTP_PORT".to_owned());
                String::new()
            });
        let smtp_user = env::var("SMTP_USER")
            .unwrap_or_else(|_|
            {
                missing.push("SMTP_USER".to_owned());
                String::new()
            });
        let smtp_password = env::var("SMTP_PASSWORD")
            .unwrap_or_else(|_|
            {
                missing.push("SMTP_PASSWORD".to_owned());
                String::new()
            });
        let from_address = env::var("SMTP_FROM")
            .unwrap_or_else(|_|
            {
                missing.push("SMTP_FROM".to_owned());
                String::new()
            });
        let admin_address = env::var("ADMIN_EMAIL")
            .unwrap_or_else(|_|
            {
                missing.push("ADMIN_EMAIL".to_owned());
                String::new()
            });

        if !missing.is_empty()
        {
            return Err(missing);
        }

        let smtp_port = smtp_port_raw
            .parse::<u16>()
            .map_err(|_| vec!["SMTP_PORT (must be u16)".to_owned()])?;

        Ok(Self
        {
            smtp_host,
            smtp_port,
            smtp_user,
            smtp_password,
            from_address,
            admin_address,
        })
    }
}
