use crate::application::errors::AppError;

/// Validates and normalizes a phone number.
pub fn validate_phone(raw: &str) -> Result<String, AppError>
{
    let trimmed = raw.trim();
    if trimmed.is_empty()
    {
        return Err(AppError::InvalidInput("phone cannot be empty".to_owned()));
    }

    if !trimmed.chars().all(|c| c.is_ascii_digit() || c == ' ' || c == '.' || c == '-' || c == '+')
    {
        return Err(AppError::InvalidInput(
            "phone contains invalid characters".to_owned(),
        ));
    }

    let digits: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect();

    if digits.len() < 8 || digits.len() > 15
    {
        return Err(AppError::InvalidInput(
            "phone must be between 8 and 15 digits".to_owned(),
        ));
    }

    Ok(digits)
}

/// Strips angle brackets from text to prevent stored XSS.
pub fn sanitize_text(raw: &str) -> String
{
    raw.chars()
        .filter(|c| *c != '<' && *c != '>')
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Validates that a material name is reasonable.
pub fn validate_material_name(raw: &str) -> Result<String, AppError>
{
    let name = sanitize_text(raw);
    if name.is_empty()
    {
        return Err(AppError::InvalidInput(
            "material name is required".to_owned(),
        ));
    }
    if name.len() > 100
    {
        return Err(AppError::InvalidInput(
            "material name is too long".to_owned(),
        ));
    }
    Ok(name)
}

/// Validates a material color label (e.g. "Noir mat", "Rouge translucide").
pub fn validate_color(raw: &str) -> Result<String, AppError>
{
    let color = sanitize_text(raw);
    if color.is_empty()
    {
        return Err(AppError::InvalidInput(
            "material color is required".to_owned(),
        ));
    }
    if color.len() > 50
    {
        return Err(AppError::InvalidInput(
            "material color is too long".to_owned(),
        ));
    }
    Ok(color)
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn valid_phone()
    {
        assert_eq!(validate_phone("06 12 34 56 78").ok(), Some("0612345678".to_owned()));
        assert_eq!(validate_phone("+33612345678").ok(), Some("+33612345678".to_owned()));
    }

    #[test]
    fn phone_rejects_letters()
    {
        assert!(validate_phone("06abcd1234").is_err());
    }

    #[test]
    fn phone_rejects_too_short()
    {
        assert!(validate_phone("123").is_err());
    }

    #[test]
    fn sanitize_strips_brackets()
    {
        assert_eq!(sanitize_text("Hello <b>world</b>"), "Hello bworld/b");
    }

    #[test]
    fn valid_color()
    {
        assert_eq!(validate_color("Noir mat").ok(), Some("Noir mat".to_owned()));
    }

    #[test]
    fn color_rejects_empty()
    {
        assert!(validate_color("   ").is_err());
    }

    #[test]
    fn color_rejects_too_long()
    {
        let long = "x".repeat(60);
        assert!(validate_color(&long).is_err());
    }
}
