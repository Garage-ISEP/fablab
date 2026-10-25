use crate::application::errors::AppError;

/// Validates and normalizes a phone number.
pub fn validate_phone(raw: &str) -> Result<String, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("phone cannot be empty".to_owned()));
    }

    if !trimmed
        .chars()
        .all(|c| c.is_ascii_digit() || c == ' ' || c == '.' || c == '-' || c == '+')
    {
        return Err(AppError::InvalidInput(
            "phone contains invalid characters".to_owned(),
        ));
    }

    let digits: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect();

    if digits.len() < 8 || digits.len() > 15 {
        return Err(AppError::InvalidInput(
            "phone must be between 8 and 15 digits".to_owned(),
        ));
    }

    Ok(digits)
}

/// Strips angle brackets from text to prevent stored XSS.
pub fn sanitize_text(raw: &str) -> String {
    raw.chars()
        .filter(|c| *c != '<' && *c != '>')
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Validates a Bambu access token pasted by the admin: printable ASCII
/// without spaces, of a plausible length. Its validity is then checked
/// against the Bambu API.
pub fn validate_bambu_token(raw: &str) -> Result<String, AppError> {
    let token = raw.trim();
    if token.is_empty() {
        return Err(AppError::InvalidInput("bambu token is required".to_owned()));
    }
    if token.len() > 4096 || !token.chars().all(|c| c.is_ascii_graphic()) {
        return Err(AppError::InvalidInput(
            "bambu token is malformed".to_owned(),
        ));
    }
    Ok(token.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_phone() {
        assert_eq!(
            validate_phone("06 12 34 56 78").ok(),
            Some("0612345678".to_owned())
        );
        assert_eq!(
            validate_phone("+33612345678").ok(),
            Some("+33612345678".to_owned())
        );
    }

    #[test]
    fn phone_rejects_letters() {
        assert!(validate_phone("06abcd1234").is_err());
    }

    #[test]
    fn phone_rejects_too_short() {
        assert!(validate_phone("123").is_err());
    }

    #[test]
    fn sanitize_strips_brackets() {
        assert_eq!(sanitize_text("Hello <b>world</b>"), "Hello bworld/b");
    }

    #[test]
    fn bambu_token_is_trimmed() {
        assert_eq!(
            validate_bambu_token("  abc-DEF_123.x \n").ok(),
            Some("abc-DEF_123.x".to_owned())
        );
    }

    #[test]
    fn bambu_token_rejects_empty() {
        assert!(validate_bambu_token("   ").is_err());
    }

    #[test]
    fn bambu_token_rejects_inner_spaces_and_non_ascii() {
        assert!(validate_bambu_token("abc def").is_err());
        assert!(validate_bambu_token("abcé").is_err());
    }
}
