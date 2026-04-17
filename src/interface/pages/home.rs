use leptos::prelude::*;

use crate::interface::components::flash_message::FlashMessage;

#[server]
pub async fn get_session_info() -> Result<Option<(String, Option<i64>)>, ServerFnError>
{
    use tower_sessions::Session;
    let session: Session = leptos_axum::extract().await?;

    let role: Option<String> = session
        .get("role")
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;

    match role
    {
        Some(r) =>
        {
            let user_id: Option<i64> = session
                .get("user_id")
                .await
                .map_err(|e| ServerFnError::new(format!("{e}")))?;
            Ok(Some((r, user_id)))
        }
        None => Ok(None),
    }
}

#[component]
pub fn HomePage() -> impl IntoView
{
    let session_info = Resource::new(|| (), |_| get_session_info());

    view!
    {
        <div class="hero">
            <div class="hero-content">
                <h1>"Impression 3D par Garage Isep"</h1>
                <p class="hero-subtitle">
                    "Le service d'impression 3D pour les etudiants Isep. \
                     Deposez vos fichiers, choisissez votre materiau, recuperez votre piece."
                </p>

                <FlashMessage />

                <Suspense fallback=|| view! { <div class="hero-actions"></div> }>
                    {move ||
                    {
                        session_info.get().map(|result|
                        {
                            match result
                            {
                                Ok(Some((role, _))) if role == "admin" =>
                                {
                                    view!
                                    {
                                        <div class="hero-actions">
                                            <a href="/admin" class="btn btn-primary btn-lg">"Tableau de bord"</a>
                                            <a href="/admin/materials" class="btn btn-lg">"Materiaux"</a>
                                        </div>
                                    }.into_any()
                                }
                                Ok(Some((_, _))) =>
                                {
                                    view!
                                    {
                                        <div class="hero-actions">
                                            <a href="/order/new" class="btn btn-primary btn-lg">"Nouvelle commande"</a>
                                            <a href="/my-orders" class="btn btn-lg">"Mes commandes"</a>
                                        </div>
                                    }.into_any()
                                }
                                _ =>
                                {
                                    view!
                                    {
                                        <div class="hero-actions">
                                            <a href="/auth/cas" class="btn btn-primary btn-lg">
                                                "Se connecter avec mon compte Isep"
                                            </a>
                                        </div>
                                    }.into_any()
                                }
                            }
                        })
                    }}
                </Suspense>
            </div>
        </div>
    }
}
