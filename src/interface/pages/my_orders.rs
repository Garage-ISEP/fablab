use leptos::prelude::*;

use crate::application::dtos::order_output::OrderView;
use crate::interface::components::flash_message::FlashMessage;
use crate::interface::components::order_table::OrderTable;
use crate::interface::session_helpers::extract_student_caller;

#[server]
pub async fn fetch_my_orders() -> Result<Vec<OrderView>, ServerFnError>
{
    let caller = extract_student_caller().await?;

    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();
    state.list_orders
        .execute(&caller)
        .map_err(|e| ServerFnError::new(format!("{e}")))
}

#[component]
pub fn MyOrdersPage() -> impl IntoView
{
    let orders = Resource::new(|| (), |_| fetch_my_orders());

    view!
    {
        <div class="page-card">
            <div class="page-header">
                <h1>"Mes commandes"</h1>
                <a href="/order/new" class="btn btn-primary">"Nouvelle commande"</a>
            </div>

            <FlashMessage />

            <Suspense fallback=|| view! { <p class="loading">"Chargement..."</p> }>
                {move ||
                {
                    orders.get().map(|result|
                    {
                        match result
                        {
                            Ok(order_list) if order_list.is_empty() =>
                            {
                                view!
                                {
                                    <div class="empty-state">
                                        <p>"Vous n'avez encore passe aucune commande."</p>
                                        <a href="/order/new" class="btn btn-primary">"Passer ma premiere commande"</a>
                                    </div>
                                }.into_any()
                            }
                            Ok(order_list) =>
                            {
                                view! { <OrderTable orders=order_list is_admin=false /> }.into_any()
                            }
                            Err(_) =>
                            {
                                view!
                                {
                                    <div class="alert alert-error">
                                        <p>"Connectez-vous pour voir vos commandes."</p>
                                        <a href="/auth/cas" class="btn btn-primary">"Connexion ISEP"</a>
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
