use tower_sessions::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashLevel
{
    Success,
    Error,
    Warning,
    Info,
}

impl FlashLevel
{
    pub fn as_str(&self) -> &'static str
    {
        match self
        {
            FlashLevel::Success => "success",
            FlashLevel::Error => "error",
            FlashLevel::Warning => "warning",
            FlashLevel::Info => "info",
        }
    }
}

pub async fn set_flash(
    session: &Session,
    level: FlashLevel,
    message: &str,
) -> Result<(), tower_sessions::session::Error>
{
    session.insert("_flash_level", level.as_str()).await?;
    session.insert("_flash_message", message).await?;
    Ok(())
}

pub async fn take_flash(
    session: &Session,
) -> Result<Option<(String, String)>, tower_sessions::session::Error>
{
    let level: Option<String> = session.get("_flash_level").await?;
    let message: Option<String> = session.get("_flash_message").await?;

    match (level, message)
    {
        (Some(l), Some(m)) =>
        {
            session.remove::<String>("_flash_level").await?;
            session.remove::<String>("_flash_message").await?;
            Ok(Some((l, m)))
        }
        _ => Ok(None),
    }
}
