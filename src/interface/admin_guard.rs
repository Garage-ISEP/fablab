use axum::extract::Request;
use axum::http::Method;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use tower_sessions::Session;

/// Page-level access guard for /admin routes.
///
/// Only inspects GET requests on /admin pages (not /admin/login).
/// POST endpoints (server functions, form actions) keep their own
/// per-handler authorization checks via `extract_admin_caller` and
/// the explicit role check in the file/order handlers.
///
/// Behavior:
/// - not authenticated -> redirect to /admin/login
/// - authenticated but not admin -> redirect to /
/// - admin -> pass through
pub async fn admin_page_guard(session: Session, request: Request, next: Next) -> Response
{
    if *request.method() != Method::GET
    {
        return next.run(request).await;
    }

    let path = request.uri().path();
    let is_admin_page = path == "/admin"
        || (path.starts_with("/admin/") && !path.starts_with("/admin/login"));
    if !is_admin_page
    {
        return next.run(request).await;
    }

    let role: Option<String> = session.get("role").await.ok().flatten();
    match role.as_deref()
    {
        Some("admin") => next.run(request).await,
        Some(_) => Redirect::to("/").into_response(),
        None => Redirect::to("/admin/login").into_response(),
    }
}
