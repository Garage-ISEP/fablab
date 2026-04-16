use leptos::prelude::ServerFnError;
use tower_sessions::Session;

use crate::application::dtos::caller::Caller;

pub async fn extract_caller() -> Result<Caller, ServerFnError>
{
    let session: Session = leptos_axum::extract().await?;

    let role: Option<String> = session
        .get("role")
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;

    match role.as_deref()
    {
        Some("admin") => Ok(Caller::Admin),
        Some("student") =>
        {
            let user_id: i64 = session
                .get("user_id")
                .await
                .map_err(|e| ServerFnError::new(format!("{e}")))?
                .ok_or_else(|| ServerFnError::new("session invalide"))?;
            Ok(Caller::Student { user_id })
        }
        _ => Err(ServerFnError::new("non authentifie")),
    }
}

pub async fn extract_admin_caller() -> Result<Caller, ServerFnError>
{
    let caller = extract_caller().await?;
    if !caller.is_admin()
    {
        return Err(ServerFnError::new("non autorise"));
    }
    Ok(caller)
}

pub async fn extract_student_caller() -> Result<Caller, ServerFnError>
{
    let caller = extract_caller().await?;
    match caller
    {
        Caller::Student { .. } => Ok(caller),
        Caller::Admin => Err(ServerFnError::new("non autorise")),
    }
}

pub async fn get_csrf_token() -> Result<String, ServerFnError>
{
    let session: Session = leptos_axum::extract().await?;
    crate::interface::csrf::ensure_csrf_token(&session)
        .await
        .map_err(|_| ServerFnError::new("csrf token error"))
}
