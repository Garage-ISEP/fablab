use quick_xml::events::Event;
use quick_xml::Reader;

use crate::domain::errors::DomainError;
use crate::domain::user::CasUser;

pub fn parse_cas_response(xml: &str) -> Result<CasUser, DomainError>
{
    let mut reader = Reader::from_str(xml);

    let mut cas_login: Option<String> = None;
    let mut display_name: Option<String> = None;
    let mut email: Option<String> = None;
    let mut promo: Option<String> = None;

    let mut current_tag = String::new();
    let mut in_auth_success = false;
    let mut found_failure = false;

    loop
    {
        match reader.read_event()
        {
            Ok(Event::Start(ref e)) =>
            {
                let tag = strip_ns(&String::from_utf8_lossy(e.name().as_ref()));

                match tag.as_str()
                {
                    "authenticationSuccess" => { in_auth_success = true; }
                    "authenticationFailure" => { found_failure = true; }
                    _ => {}
                }

                if in_auth_success
                {
                    current_tag = tag;
                }
            }
            Ok(Event::Text(ref e)) if in_auth_success =>
            {
                let text = e
                    .decode()
                    .map_err(|err| DomainError::Validation(format!("CAS XML text decode error: {err}")))?
                    .into_owned();

                match current_tag.as_str()
                {
                    "user" => cas_login = Some(text),
                    "displayName" => display_name = Some(text),
                    "mail" => email = Some(text),
                    "titre" => promo = Some(text),
                    _ => {}
                }
            }
            Ok(Event::End(_)) => { current_tag.clear(); }
            Ok(Event::Eof) => break,
            Err(e) =>
            {
                return Err(DomainError::Validation(format!("CAS XML parse error: {e}")));
            }
            _ => {}
        }
    }

    if found_failure
    {
        return Err(DomainError::Validation(
            "CAS authentication failed: invalid ticket".to_owned(),
        ));
    }

    let cas_login = cas_login
        .ok_or_else(|| DomainError::Validation("CAS response missing user field".to_owned()))?;
    let display_name = display_name
        .ok_or_else(|| DomainError::Validation("CAS response missing displayName".to_owned()))?;
    let email = email
        .ok_or_else(|| DomainError::Validation("CAS response missing mail".to_owned()))?;

    Ok(CasUser { cas_login, display_name, email, promo })
}

fn strip_ns(tag: &str) -> String
{
    match tag.find(':')
    {
        Some(pos) => tag[pos + 1..].to_owned(),
        None => tag.to_owned(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests
{
    use super::*;

    #[test]
    fn test_parse_success()
    {
        let xml = r#"<cas:serviceResponse xmlns:cas="http://www.yale.edu/tp/cas">
            <cas:authenticationSuccess>
                <cas:user>situ62394</cas:user>
                <cas:attributes>
                    <cas:displayName>Simon TULOUP</cas:displayName>
                    <cas:mail>situ62394@eleve.isep.fr</cas:mail>
                    <cas:titre>Paris-ING-A2-2526</cas:titre>
                </cas:attributes>
            </cas:authenticationSuccess>
        </cas:serviceResponse>"#;

        let user = parse_cas_response(xml).unwrap();
        assert_eq!(user.cas_login, "situ62394");
        assert_eq!(user.display_name, "Simon TULOUP");
        assert_eq!(user.email, "situ62394@eleve.isep.fr");
        assert_eq!(user.promo.as_deref(), Some("Paris-ING-A2-2526"));
    }

    #[test]
    fn test_parse_failure()
    {
        let xml = r#"<cas:serviceResponse xmlns:cas="http://www.yale.edu/tp/cas">
            <cas:authenticationFailure code="INVALID_TICKET">Invalid</cas:authenticationFailure>
        </cas:serviceResponse>"#;

        assert!(parse_cas_response(xml).is_err());
    }

    #[test]
    fn test_parse_malformed()
    {
        assert!(parse_cas_response("<not valid xml at all").is_err());
    }
}
