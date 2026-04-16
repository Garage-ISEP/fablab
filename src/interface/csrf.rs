use axum::body::Body;
use axum::extract::Request;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tower_sessions::Session;

const CSRF_TOKEN_KEY: &str = "_csrf_token";

fn generate_token() -> String
{
    use std::fmt::Write;
    let mut buf = [0u8; 16];
    getrandom::fill(&mut buf).expect("getrandom failed");
    let mut hex = String::with_capacity(32);
    for byte in &buf
    {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

pub async fn ensure_csrf_token(session: &Session) -> Result<String, StatusCode>
{
    let existing: Option<String> = session
        .get(CSRF_TOKEN_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match existing
    {
        Some(token) => Ok(token),
        None =>
        {
            let token = generate_token();
            session
                .insert(CSRF_TOKEN_KEY, &token)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(token)
        }
    }
}

pub async fn csrf_middleware(session: Session, request: Request, next: Next) -> Response
{
    let method = request.method().clone();

    if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS
    {
        let _ = ensure_csrf_token(&session).await;
        return next.run(request).await;
    }

    let session_token: Option<String> = match session.get(CSRF_TOKEN_KEY).await
    {
        Ok(t) => t,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let session_token = match session_token
    {
        Some(t) => t,
        None => return (StatusCode::FORBIDDEN, "Missing CSRF session").into_response(),
    };

    let header_token = request
        .headers()
        .get("X-CSRF-Token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());

    if let Some(ht) = &header_token
        && constant_time_eq(ht.as_bytes(), session_token.as_bytes())
    {
        return next.run(request).await;
    }

    // Multipart uploads: do not buffer the body here. The upload handler
    // parses the _csrf field as the first multipart part and validates
    // it against the session token itself.
    let is_multipart = request
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.starts_with("multipart/form-data"));
    if is_multipart
    {
        return next.run(request).await;
    }

    let (parts, body) = request.into_parts();

    let body_bytes = match axum::body::to_bytes(body, 1024 * 1024).await
    {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let form_token = extract_csrf_from_form(&body_bytes);

    let valid = match form_token
    {
        Some(ref ft) => constant_time_eq(ft.as_bytes(), session_token.as_bytes()),
        None => false,
    };

    if !valid
    {
        return (StatusCode::FORBIDDEN, "Invalid CSRF token").into_response();
    }

    let request = Request::from_parts(parts, Body::from(body_bytes));
    next.run(request).await
}

/// Shared helper for handlers (notably multipart upload) that need to
/// validate a CSRF token already extracted from the request body.
pub async fn validate_csrf_token(session: &Session, candidate: &str) -> bool
{
    let stored: Option<String> = match session.get(CSRF_TOKEN_KEY).await
    {
        Ok(t) => t,
        Err(_) => return false,
    };
    match stored
    {
        Some(s) => constant_time_eq(candidate.as_bytes(), s.as_bytes()),
        None => false,
    }
}

fn extract_csrf_from_form(body: &[u8]) -> Option<String>
{
    let body_str = std::str::from_utf8(body).ok()?;
    for pair in body_str.split('&')
    {
        if let Some(value) = pair.strip_prefix("_csrf=")
        {
            return Some(
                urlencoding::decode(value)
                    .unwrap_or_else(|_| value.into())
                    .into_owned(),
            );
        }
    }
    None
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool
{
    if a.len() != b.len()
    {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter())
    {
        diff |= x ^ y;
    }
    diff == 0
}
