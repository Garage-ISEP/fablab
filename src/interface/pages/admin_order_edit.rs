use leptos::prelude::*;
use leptos::form::ActionForm;
use serde::{Deserialize, Serialize};

use crate::application::dtos::order_output::OrderView;
use crate::domain::material::Material;
use crate::interface::components::csrf_field::CsrfField;
use crate::interface::components::flash_message::FlashMessage;
use crate::interface::session_helpers::extract_admin_caller;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderEditContext
{
    pub order: OrderView,
    pub available_materials: Vec<Material>,
}

#[server]
pub async fn fetch_order_edit_context(order_id: i64) -> Result<OrderEditContext, ServerFnError>
{
    let caller = extract_admin_caller().await?;

    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();
    let get_order = state.get_order.clone();
    let list_materials = state.list_materials.clone();

    tokio::task::spawn_blocking(move ||
    {
        let order = get_order
            .execute(order_id, &caller)
            .map_err(|e| ServerFnError::new(format!("{e}")))?;
        let materials = list_materials
            .execute(true)
            .map_err(|e| ServerFnError::new(format!("{e}")))?;
        Ok::<_, ServerFnError>(OrderEditContext
        {
            order,
            available_materials: materials,
        })
    })
    .await
    .map_err(|e| ServerFnError::new(format!("{e}")))?
}

#[server]
pub async fn update_order_action(
    order_id: String,
    status: String,
    requires_payment: Option<String>,
    sliced_weight_grams: String,
    print_time_minutes: String,
    material_id: String,
) -> Result<(), ServerFnError>
{
    use tower_sessions::Session;
    use crate::application::dtos::order_input::UpdateOrderInput;
    use crate::interface::flash::{FlashLevel, set_flash};
    use crate::interface::error_messages::user_message;

    let caller = extract_admin_caller().await?;
    let session: Session = leptos_axum::extract().await?;

    let oid = order_id.parse::<i64>()
        .map_err(|_| ServerFnError::new("id invalide"))?;

    let material_id_opt = match material_id.trim()
    {
        "" => None,
        s => s.parse::<i64>().ok().filter(|&n| n > 0),
    };

    let input = UpdateOrderInput
    {
        order_id: oid,
        status: if status.is_empty() { None } else { Some(status) },
        requires_payment: Some(requires_payment.is_some()),
        sliced_weight_grams: sliced_weight_grams.parse::<f64>().ok(),
        print_time_minutes: print_time_minutes.parse::<i32>().ok(),
        material_id: material_id_opt,
    };

    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();
    match state.update_order.execute(input, &caller).await
    {
        Ok(_) =>
        {
            let _ = set_flash(&session, FlashLevel::Success, "Commande mise a jour.").await;
        }
        Err(e) =>
        {
            let msg = user_message(&e);
            let _ = set_flash(&session, FlashLevel::Error, msg).await;
        }
    }

    leptos_axum::redirect(&format!("/admin/order/{oid}"));
    Ok(())
}

#[component]
pub fn AdminOrderEditPage() -> impl IntoView
{
    let params = leptos_router::hooks::use_params_map();
    let order_id = move ||
    {
        params.get().get("id")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    };

    let ctx = Resource::new(order_id, fetch_order_edit_context);

    view!
    {
        <div class="page-card">
            <FlashMessage />
            <Suspense fallback=|| view! { <p class="loading">"Chargement..."</p> }>
                {move ||
                {
                    ctx.get().map(|result|
                    {
                        match result
                        {
                            Ok(c) => render_order_detail(c.order, c.available_materials).into_any(),
                            Err(_) =>
                            {
                                view!
                                {
                                    <div class="alert alert-error">
                                        <p>"Commande introuvable ou acces refuse."</p>
                                        <a href="/admin" class="btn">"Retour"</a>
                                    </div>
                                }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}

fn render_order_detail(o: OrderView, available_materials: Vec<Material>) -> impl IntoView
{
    let files = o.files.clone();
    let oid = o.id.to_string();
    let oid_for_delete = oid.clone();
    let weight = o.sliced_weight_grams.map(|v| v.to_string()).unwrap_or_default();
    let time = o.print_time_minutes.map(|v| v.to_string()).unwrap_or_default();
    let is_a_traiter = o.status == "a_traiter";
    let is_en_traitement = o.status == "en_traitement";
    let is_imprime = o.status == "imprime";
    let is_livre = o.status == "livre";
    let is_annule = o.status == "annule";
    let order_is_terminal = is_livre || is_annule;

    let display_name = o.user_display_name.clone();
    let material_display = o.material_label.clone().unwrap_or_else(|| "Pas de preference".to_owned());
    let comments_val = o.comments.clone().unwrap_or_else(|| "-".to_owned());
    let software = o.software_used.clone();
    let date = o.created_at.clone();

    let order_has_material = o.material_id.is_some();
    let current_material_id = o.material_id;

    let update_action = ServerAction::<UpdateOrderAction>::new();
    let delete_order_action = format!("/admin/order/{oid_for_delete}/delete");

    view!
    {
        <div class="page-header">
            <h1>"Commande #"{o.id}</h1>
            <a href="/admin" class="btn">"Retour"</a>
        </div>

        <div class="order-detail-grid">
            <section class="detail-card">
                <h2>"Informations"</h2>
                <dl class="detail-list">
                    <dt>"Client"</dt><dd>{display_name}</dd>
                    <dt>"Date"</dt><dd>{date}</dd>
                    <dt>"Logiciel"</dt><dd>{software}</dd>
                    <dt>"Materiau actuel"</dt><dd>{material_display}</dd>
                    <dt>"Quantite"</dt><dd>{o.quantity}</dd>
                    <dt>"Commentaires"</dt><dd>{comments_val}</dd>
                </dl>

                <h3>"Fichiers"</h3>
                <ul class="file-list">
                    {files.into_iter().map(|f|
                    {
                        let href = format!("/admin/files/{}/download", f.id);
                        let size_kb = (f.size_bytes + 1023) / 1024;
                        let name = f.original_filename.clone();
                        let del_action = format!("/admin/files/{}/delete", f.id);
                        let confirm_msg = if order_is_terminal
                        {
                            "Supprimer ce fichier ?"
                        }
                        else
                        {
                            "Attention: cette commande n'est ni livree ni annulee. Supprimer ce fichier maintenant peut bloquer le traitement. Continuer ?"
                        };
                        let onsubmit = format!("return confirm('{}');", confirm_msg.replace('\'', "\\'"));
                        view!
                        {
                            <li>
                                <a href=href class="file-link">{name}</a>
                                " "
                                <span class="file-size">{format!("({size_kb} KB)")}</span>
                                " "
                                <form method="post" action=del_action style="display:inline;" onsubmit=onsubmit>
                                    <CsrfField />
                                    <button type="submit" class="btn btn-sm btn-danger">"Supprimer"</button>
                                </form>
                            </li>
                        }
                    }).collect::<Vec<_>>()}
                </ul>

                <h3>"Zone dangereuse"</h3>
                <form method="post" action=delete_order_action
                    onsubmit="return confirm('Supprimer definitivement cette commande et tous ses fichiers ? Cette action est irreversible.');">
                    <CsrfField />
                    <button type="submit" class="btn btn-danger">"Supprimer la commande"</button>
                </form>
            </section>

            <section class="detail-card">
                <h2>"Edition"</h2>
                <ActionForm action=update_action>
                    <CsrfField />
                    <input type="hidden" name="order_id" value=oid />

                    <div class="form-group">
                        <label for="status">"Statut"</label>
                        <select id="status" name="status">
                            <option value="a_traiter" selected=is_a_traiter>"A traiter"</option>
                            <option value="en_traitement" selected=is_en_traitement>"En traitement"</option>
                            <option value="imprime" selected=is_imprime>"Imprime"</option>
                            <option value="livre" selected=is_livre>"Livre"</option>
                            <option value="annule" selected=is_annule>"Annule"</option>
                        </select>
                    </div>

                    <div class="form-group">
                        <label for="material_id">"Materiau"</label>
                        <select id="material_id" name="material_id">
                            {(!order_has_material).then(||
                            {
                                view!
                                {
                                    <option value="" selected=true>"-- Pas de preference --"</option>
                                }
                            })}
                            {
                                let mut options: Vec<_> = available_materials
                                    .iter()
                                    .map(|m| (m.id, m.label()))
                                    .collect();
                                if let Some(cur_id) = current_material_id
                                {
                                    let already_listed = options.iter().any(|(id, _)| *id == cur_id);
                                    if !already_listed
                                        && let Some(current_label) = o.material_label.clone()
                                        {
                                            options.insert(0, (cur_id, format!("{current_label} (indisponible)")));
                                        }
                                }
                                options.into_iter().map(|(mid, label)|
                                {
                                    let mid_str = mid.to_string();
                                    let selected = current_material_id == Some(mid);
                                    view!
                                    {
                                        <option value=mid_str selected=selected>{label}</option>
                                    }
                                }).collect::<Vec<_>>()
                            }
                        </select>
                    </div>

                    <div class="form-row">
                        <div class="form-group">
                            <label for="sliced_weight_grams">"Poids (g)"</label>
                            <input id="sliced_weight_grams" name="sliced_weight_grams" type="number" step="0.01" value=weight />
                        </div>
                        <div class="form-group">
                            <label for="print_time_minutes">"Temps (min)"</label>
                            <input id="print_time_minutes" name="print_time_minutes" type="number" value=time />
                        </div>
                    </div>

                    <div class="form-group checkbox-group">
                        <label>
                            <input type="checkbox" name="requires_payment" checked=o.requires_payment />
                            "Cette commande necessite un paiement (a traiter manuellement)"
                        </label>
                    </div>

                    <button type="submit" class="btn btn-primary">"Sauvegarder"</button>
                </ActionForm>
            </section>
        </div>
    }
}
