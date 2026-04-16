use leptos::prelude::*;
use leptos::server_fn::ServerFn;

use crate::domain::material::Material;

use crate::interface::components::csrf_field::CsrfField;
use crate::interface::components::flash_message::FlashMessage;
use crate::interface::session_helpers::extract_admin_caller;

#[server]
pub async fn fetch_all_materials() -> Result<Vec<Material>, ServerFnError>
{
    let _caller = extract_admin_caller().await?;

    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();
    state.list_materials
        .execute(false)
        .map_err(|e| ServerFnError::new(format!("{e}")))
}

#[server]
pub async fn upsert_material_action(
    id: String,
    name: String,
    color: String,
    available: Option<String>,
) -> Result<(), ServerFnError>
{
    use tower_sessions::Session;
    use crate::interface::flash::{FlashLevel, set_flash};
    use crate::application::validation;

    let _caller = extract_admin_caller().await?;
    let session: Session = leptos_axum::extract().await?;

    let parsed_id = id.parse::<i64>()
        .map_err(|_| ServerFnError::new("id invalide"))?;

    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();

    let final_id = if parsed_id <= 0
    {
        state.manage_material
            .next_id()
            .map_err(|e| ServerFnError::new(format!("{e}")))?
    }
    else
    {
        parsed_id
    };

    let validated_name = match validation::validate_material_name(&name)
    {
        Ok(n) => n,
        Err(_) =>
        {
            let _ = set_flash(&session, FlashLevel::Error, "Le nom du materiau est requis.").await;
            leptos_axum::redirect("/admin/materials");
            return Ok(());
        }
    };

    let validated_color = match validation::validate_color(&color)
    {
        Ok(c) => c,
        Err(_) =>
        {
            let _ = set_flash(&session, FlashLevel::Error, "La couleur est requise.").await;
            leptos_axum::redirect("/admin/materials");
            return Ok(());
        }
    };

    let material = Material
    {
        id: final_id,
        name: validated_name,
        color: validated_color,
        available: available.is_some(),
    };

    match state.manage_material.execute(material)
    {
        Ok(_) =>
        {
            let _ = set_flash(&session, FlashLevel::Success, "Materiau enregistre.").await;
        }
        Err(_) =>
        {
            let _ = set_flash(&session, FlashLevel::Error, "Erreur lors de l'enregistrement.").await;
        }
    }

    leptos_axum::redirect("/admin/materials");
    Ok(())
}

#[component]
pub fn AdminMaterialsPage() -> impl IntoView
{
    let materials = Resource::new(|| (), |_| fetch_all_materials());

    view!
    {
        <div class="page-card">
            <div class="page-header">
                <h1>"Materiaux"</h1>
            </div>

            <FlashMessage />

            <Suspense fallback=|| view! { <p class="loading">"Chargement..."</p> }>
                {move ||
                {
                    materials.get().map(|result|
                    {
                        match result
                        {
                            Ok(mats) if mats.is_empty() =>
                            {
                                view!
                                {
                                    <div class="empty-state">
                                        <p>"Aucun materiau enregistre."</p>
                                    </div>
                                }.into_any()
                            }
                            Ok(mats) =>
                            {
                                view!
                                {
                                    <table class="data-table">
                                        <thead>
                                            <tr>
                                                <th>"Nom"</th>
                                                <th>"Couleur"</th>
                                                <th>"Disponible"</th>
                                                <th>"Actions"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {mats.into_iter().map(render_material_row).collect::<Vec<_>>()}
                                        </tbody>
                                    </table>
                                }.into_any()
                            }
                            Err(_) =>
                            {
                                view!
                                {
                                    <div class="alert alert-error">
                                        <p>"Acces refuse."</p>
                                        <a href="/admin/login" class="btn btn-primary">"Se connecter"</a>
                                    </div>
                                }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>

            <section class="add-material-section">
                <h2>"Ajouter un materiau"</h2>
                <form method="post" action=UpsertMaterialAction::url() class="material-form">
                    <CsrfField />
                    <input type="hidden" name="id" value="-1" />
                    <div class="form-row">
                        <div class="form-group">
                            <label for="new-name">"Nom"</label>
                            <input id="new-name" name="name" type="text" required placeholder="PLA, ABS, PETG..." />
                        </div>
                        <div class="form-group">
                            <label for="new-color">"Couleur"</label>
                            <input id="new-color" name="color" type="text" required placeholder="Noir mat, Rouge translucide..." />
                        </div>
                    </div>
                    <div class="form-group checkbox-group">
                        <label>
                            <input type="checkbox" name="available" checked />
                            "Disponible"
                        </label>
                    </div>
                    <button type="submit" class="btn btn-primary">"Ajouter"</button>
                </form>
            </section>
        </div>
    }
}

fn render_material_row(m: Material) -> impl IntoView
{
    let mid = m.id.to_string();
    let delete_action = format!("/admin/materials/{}/delete", m.id);
    let name = m.name.clone();
    let color = m.color.clone();
    let toggle_checked = !m.available;

    view!
    {
        <tr>
            <td class="td-name">{name.clone()}</td>
            <td>{color.clone()}</td>
            <td>
                {if m.available
                {
                    view! { <span class="badge badge-delivered">"Oui"</span> }.into_any()
                }
                else
                {
                    view! { <span class="badge badge-cancelled">"Non"</span> }.into_any()
                }}
            </td>
            <td>
                <form method="post" action=UpsertMaterialAction::url() style="display:inline">
                    <CsrfField />
                    <input type="hidden" name="id" value=mid />
                    <input type="hidden" name="name" value=name />
                    <input type="hidden" name="color" value=color />
                    {if toggle_checked
                    {
                        Some(view! { <input type="hidden" name="available" value="on" /> })
                    }
                    else
                    {
                        None
                    }}
                    <button type="submit" class="btn btn-small">
                        {if m.available { "Desactiver" } else { "Activer" }}
                    </button>
                </form>
                <form
                    method="post"
                    action=delete_action
                    style="display:inline; margin-left: 0.5rem"
                    onsubmit="return confirm('Supprimer ce materiau ? Cette action est irreversible.')"
                >
                    <CsrfField />
                    <button type="submit" class="btn btn-small btn-danger">"Supprimer"</button>
                </form>
            </td>
        </tr>
    }
}