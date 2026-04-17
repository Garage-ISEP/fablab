use leptos::prelude::*;

use crate::domain::material::Material;

use crate::interface::components::flash_message::FlashMessage;
use crate::interface::session_helpers::get_csrf_token;

#[server]
pub async fn fetch_available_materials() -> Result<Vec<Material>, ServerFnError>
{
    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();
    let list_materials = state.list_materials.clone();
    tokio::task::spawn_blocking(move ||
    {
        list_materials
            .execute(true)
            .map_err(|e| ServerFnError::new(format!("{e}")))
    })
    .await
    .map_err(|e| ServerFnError::new(format!("{e}")))?
}

#[server]
pub async fn fetch_user_phone() -> Result<Option<String>, ServerFnError>
{
    use crate::interface::session_helpers::extract_caller;

    let caller = extract_caller().await?;
    let user_id = match caller
    {
        crate::application::dtos::caller::Caller::Student { user_id } => user_id,
        _ => return Ok(None),
    };

    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();
    let get_user_phone = state.get_user_phone.clone();
    tokio::task::spawn_blocking(move ||
    {
        get_user_phone
            .execute(user_id)
            .map_err(|e| ServerFnError::new(format!("{e}")))
    })
    .await
    .map_err(|e| ServerFnError::new(format!("{e}")))?
}

#[server]
pub async fn fetch_form_bootstrap()
    -> Result<(Vec<Material>, Option<String>, String), ServerFnError>
{
    let materials = fetch_available_materials().await?;
    let phone = fetch_user_phone().await?;
    let csrf = get_csrf_token().await?;
    Ok((materials, phone, csrf))
}

#[component]
pub fn OrderFormPage() -> impl IntoView
{
    let bootstrap = Resource::new(|| (), |_| fetch_form_bootstrap());

    view!
    {
        <div class="page-card">
            <div class="page-header">
                <h1>"Nouvelle commande"</h1>
                <a href="/my-orders" class="btn">"Mes commandes"</a>
            </div>

            <FlashMessage />

            <Suspense fallback=|| view! { <p class="loading">"Chargement..."</p> }>
                {move ||
                {
                    bootstrap.get().map(|result|
                    {
                        let (mats, phone, csrf) = result.unwrap_or_else(|_|
                            (Vec::new(), None, String::new()));
                        let user_phone = phone.unwrap_or_default();

                        view!
                        {
                            <form method="post" action="/orders" enctype="multipart/form-data" class="order-form">
                                <input type="hidden" name="_csrf" value=csrf />

                                <div class="form-row">
                                    <div class="form-group">
                                        <label for="software_used">"Logiciel utilise"</label>
                                        <input id="software_used" name="software_used" type="text" required
                                            placeholder="Fusion 360, Blender..." />
                                    </div>
                                    <div class="form-group">
                                        <label for="material_id">"Materiau et couleur"</label>
                                        <select id="material_id" name="material_id">
                                            <option value="">"Pas de preference"</option>
                                            {mats.into_iter().map(|m|
                                            {
                                                let id_str = m.id.to_string();
                                                let label = m.label();
                                                view! { <option value=id_str>{label}</option> }
                                            }).collect::<Vec<_>>()}
                                        </select>
                                    </div>
                                </div>

                                <div class="form-row">
                                    <div class="form-group">
                                        <label for="quantity">"Quantite"</label>
                                        <select id="quantity" name="quantity">
                                            {(1..=10).map(|n|
                                            {
                                                let ns = n.to_string();
                                                let ns2 = ns.clone();
                                                view! { <option value=ns>{ns2}</option> }
                                            }).collect::<Vec<_>>()}
                                        </select>
                                    </div>
                                    <div class="form-group">
                                        <label for="phone">"Telephone"</label>
                                        <input id="phone" name="phone" type="tel" required value=user_phone
                                            placeholder="06 12 34 56 78 ou +33 6 12 34 56 78" />
                                    </div>
                                </div>

                                <div class="form-group">
                                    <label for="comments">"Commentaires (optionnel)"</label>
                                    <textarea id="comments" name="comments" rows="2"
                                        placeholder="Precisions sur votre commande..."></textarea>
                                </div>

                                <div class="form-group">
                                    <label for="files">"Fichiers 3D (STL, 3MF, STP)"</label>
                                    <input id="files" name="files" type="file" required multiple
                                        accept=".stl,.3mf,.stp,.step" />
                                </div>

                                <button type="submit" class="btn btn-primary btn-lg">"Envoyer la commande"</button>
                            </form>
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}
