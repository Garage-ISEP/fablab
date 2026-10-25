use leptos::prelude::*;

use crate::domain::filament::ConnectionState;

#[server]
pub async fn fetch_bambu_connection() -> Result<ConnectionState, ServerFnError> {
    use crate::interface::session_helpers::extract_admin_caller;

    let _caller = extract_admin_caller().await?;
    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();
    Ok(state.printer_overview.connection())
}

/// Warns the admin when the filament levels cannot be trusted, since
/// weight validation is then blocked.
#[component]
pub fn BambuStatusBanner() -> impl IntoView {
    let connection = Resource::new(|| (), |_| fetch_bambu_connection());

    view! {
        <Suspense fallback=|| ()>
            {move ||
            {
                connection.get().and_then(|result|
                {
                    let (class, message) = match result.ok()?
                    {
                        ConnectionState::NotConfigured => (
                            "alert alert-warning",
                            "Aucun token Bambu configure : les niveaux de filament sont inconnus et la saisie des poids est bloquee.",
                        ),
                        ConnectionState::TokenRejected => (
                            "alert alert-error",
                            "Le token Bambu a expire ou a ete refuse : les niveaux de filament ne sont plus mis a jour et la saisie des poids est bloquee. Saisissez un nouveau token.",
                        ),
                        ConnectionState::Unreachable(_) => (
                            "alert alert-warning",
                            "Cloud Bambu injoignable, nouvelle tentative automatique en cours. La saisie des poids est bloquee en attendant.",
                        ),
                        ConnectionState::Connecting | ConnectionState::Connected => return None,
                    };
                    Some(view!
                    {
                        <div class=class role="alert">
                            <p>{message}</p>
                            <a href="/admin/printers" class="btn btn-small">"Gerer la connexion Bambu"</a>
                        </div>
                    })
                })
            }}
        </Suspense>
    }
}
