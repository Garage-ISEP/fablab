use leptos::prelude::*;

use crate::application::dtos::order_output::OrderView;
use crate::interface::components::flash_message::FlashMessage;
use crate::interface::components::status_badge::StatusBadge;
use crate::interface::session_helpers::extract_student_caller;

#[server]
pub async fn fetch_my_order_detail(order_id: i64) -> Result<OrderView, ServerFnError>
{
    let caller = extract_student_caller().await?;

    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();
    let get_order = state.get_order.clone();
    tokio::task::spawn_blocking(move ||
    {
        get_order
            .execute(order_id, &caller)
            .map_err(|e| ServerFnError::new(format!("{e}")))
    })
    .await
    .map_err(|e| ServerFnError::new(format!("{e}")))?
}

#[component]
pub fn MyOrderDetailPage() -> impl IntoView
{
    let params = leptos_router::hooks::use_params_map();
    let order_id = move ||
    {
        params.get().get("id")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    };

    let order = Resource::new(order_id, fetch_my_order_detail);

    view!
    {
        <div class="page-card">
            <FlashMessage />
            <Suspense fallback=|| view! { <p class="loading">"Chargement..."</p> }>
                {move ||
                {
                    order.get().map(|result|
                    {
                        match result
                        {
                            Ok(o) => render_student_order_detail(o).into_any(),
                            Err(_) =>
                            {
                                view!
                                {
                                    <div class="alert alert-error">
                                        <p>"Commande introuvable ou acces refuse."</p>
                                        <a href="/my-orders" class="btn">"Mes commandes"</a>
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

fn render_student_order_detail(o: OrderView) -> impl IntoView
{
    use crate::interface::components::csrf_field::CsrfField;

    let files = o.files.clone();
    let oid = o.id;
    let can_cancel = o.status == "a_traiter";
    let cancel_action = format!("/my-orders/{oid}/cancel");

    let status = o.status.clone();
    let material = o.material_label.clone().unwrap_or_else(|| "-".to_owned());
    let comments_val = o.comments.clone().unwrap_or_else(|| "-".to_owned());
    let software = o.software_used.clone();
    let date = o.created_at.clone();
    let req_label = if o.requires_payment
    {
        "Oui (l'admin vous contactera par email)"
    }
    else
    {
        "Non"
    };

    view!
    {
        <div class="page-header">
            <h1>"Commande #"{o.id}</h1>
            <a href="/my-orders" class="btn">"Mes commandes"</a>
        </div>

        <div class="order-detail-grid">
            <section class="detail-card">
                <h2>"Details"</h2>
                <dl class="detail-list">
                    <dt>"Date"</dt><dd>{date}</dd>
                    <dt>"Statut"</dt><dd><StatusBadge status=status /></dd>
                    <dt>"Logiciel"</dt><dd>{software}</dd>
                    <dt>"Materiau"</dt><dd>{material}</dd>
                    <dt>"Quantite"</dt><dd>{o.quantity}</dd>
                    <dt>"Paiement"</dt><dd>{req_label}</dd>
                    <dt>"Commentaires"</dt><dd>{comments_val}</dd>
                </dl>

                <h3>"Fichiers"</h3>
                <ul class="file-list">
                    {files.into_iter().map(|f|
                    {
                        let size_kb = (f.size_bytes + 1023) / 1024;
                        let name = f.original_filename.clone();
                        view!
                        {
                            <li>
                                <span class="file-name">{name}</span>
                                " "
                                <span class="file-size">{format!("({size_kb} KB)")}</span>
                            </li>
                        }
                    }).collect::<Vec<_>>()}
                </ul>

                {if can_cancel
                {
                    Some(view!
                    {
                        <h3>"Annulation"</h3>
                        <p class="muted">"Vous pouvez annuler cette commande tant qu'elle est encore a traiter. Les fichiers seront supprimes."</p>
                        <form method="post" action=cancel_action
                            onsubmit="return confirm('Annuler cette commande ? Les fichiers seront supprimes definitivement.');">
                            <CsrfField />
                            <button type="submit" class="btn btn-danger">"Annuler la commande"</button>
                        </form>
                    })
                }
                else
                {
                    None
                }}
            </section>

            {if o.sliced_weight_grams.is_some() || o.print_time_minutes.is_some()
            {
                let weight = o.sliced_weight_grams
                    .map(|w| format!("{w:.1} g"))
                    .unwrap_or_else(|| "-".to_owned());
                let time = o.print_time_minutes
                    .map(|t| format!("{t} min"))
                    .unwrap_or_else(|| "-".to_owned());

                Some(view!
                {
                    <section class="detail-card">
                        <h2>"Impression"</h2>
                        <dl class="detail-list">
                            <dt>"Poids"</dt><dd>{weight}</dd>
                            <dt>"Temps"</dt><dd>{time}</dd>
                        </dl>
                    </section>
                })
            }
            else
            {
                None
            }}
        </div>
    }
}