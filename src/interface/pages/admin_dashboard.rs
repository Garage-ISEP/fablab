use std::str::FromStr;

use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

use crate::application::dtos::order_filter::{OrderFilter, PaymentFilter};
use crate::application::dtos::order_output::OrderView;
use crate::application::dtos::order_sort::{OrderSort, SortColumn, SortDirection};
use crate::interface::components::flash_message::FlashMessage;
use crate::interface::components::order_table::OrderTable;
use crate::interface::session_helpers::extract_admin_caller;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DashboardQuery
{
    status: String,
    payment: String,
    search: String,
    sort_col: String,
    sort_dir: String,
}

impl DashboardQuery
{
    fn to_filter(&self) -> OrderFilter
    {
        OrderFilter
        {
            status: if self.status.is_empty() || self.status == "all"
            {
                None
            }
            else
            {
                Some(self.status.clone())
            },
            payment: PaymentFilter::from_str(&self.payment).ok(),
            search: if self.search.trim().is_empty()
            {
                None
            }
            else
            {
                Some(self.search.clone())
            },
        }
    }

    fn to_sort(&self) -> OrderSort
    {
        let column = match self.sort_col.as_str()
        {
            "date" => SortColumn::CreatedAt,
            "client" => SortColumn::Client,
            "material" => SortColumn::Material,
            "quantity" => SortColumn::Quantity,
            "status" => SortColumn::Status,
            "payment" => SortColumn::RequiresPayment,
            "weight" => SortColumn::Weight,
            "time" => SortColumn::PrintTime,
            _ => SortColumn::Id,
        };
        let direction = match self.sort_dir.as_str()
        {
            "asc" => SortDirection::Asc,
            _ => SortDirection::Desc,
        };
        OrderSort::new(column, direction)
    }
}

#[server]
pub async fn fetch_filtered_orders(
    status: String,
    payment: String,
    search: String,
    sort_col: String,
    sort_dir: String,
) -> Result<Vec<OrderView>, ServerFnError>
{
    let caller = extract_admin_caller().await?;
    let query = DashboardQuery { status, payment, search, sort_col, sort_dir };

    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();
    let list_orders = state.list_orders.clone();
    tokio::task::spawn_blocking(move ||
    {
        list_orders
            .execute_filtered(&caller, &query.to_filter(), query.to_sort())
            .map_err(|e| ServerFnError::new(format!("{e}")))
    })
    .await
    .map_err(|e| ServerFnError::new(format!("{e}")))?
}

#[component]
pub fn AdminDashboardPage() -> impl IntoView
{
    let query_map = use_query_map();

    // Read query params reactively. On SSR each navigation re-runs this.
    let dashboard_query = move || -> DashboardQuery
    {
        let q = query_map.get();
        DashboardQuery
        {
            status: q.get("status").unwrap_or_default(),
            payment: q.get("payment").unwrap_or_default(),
            search: q.get("q").unwrap_or_default(),
            sort_col: q.get("sort").unwrap_or_default(),
            sort_dir: q.get("dir").unwrap_or_default(),
        }
    };

    // Resource keyed on the query so it re-fetches when URL changes.
    let orders = Resource::new(
        dashboard_query,
        |dq| fetch_filtered_orders(
            dq.status, dq.payment, dq.search, dq.sort_col, dq.sort_dir,
        ),
    );

    view!
    {
        <div class="page-card">
            <div class="page-header">
                <h1>"Commandes"</h1>
            </div>

            <FlashMessage />

            {move ||
            {
                let dq = dashboard_query();
                let current_sort = dq.to_sort();
                view!
                {
                    <FilterForm query=dq.clone() />
                    <Suspense fallback=|| view! { <p class="loading">"Chargement..."</p> }>
                        {move ||
                        {
                            let dq = dq.clone();
                            orders.get().map(move |result|
                            {
                                match result
                                {
                                    Ok(list) if list.is_empty() =>
                                    {
                                        view!
                                        {
                                            <div class="empty-state">
                                                <p>"Aucune commande correspondante."</p>
                                            </div>
                                        }
                                        .into_any()
                                    }
                                    Ok(list) =>
                                    {
                                        view!
                                        {
                                            <OrderTable
                                                orders=list
                                                is_admin=true
                                                sort=current_sort
                                                sort_base_url=build_base_url(&dq)
                                            />
                                        }
                                        .into_any()
                                    }
                                    Err(_) =>
                                    {
                                        view!
                                        {
                                            <div class="alert alert-error">
                                                <p>"Acces refuse."</p>
                                                <a href="/admin/login" class="btn btn-primary">
                                                    "Se connecter"
                                                </a>
                                            </div>
                                        }
                                        .into_any()
                                    }
                                }
                            })
                        }}
                    </Suspense>
                }
            }}
        </div>
    }
}

/// Builds a base URL preserving filters but stripping sort params.
/// The OrderTable will append &sort=...&dir=... per column.
fn build_base_url(dq: &DashboardQuery) -> String
{
    let mut parts: Vec<String> = Vec::new();
    if !dq.status.is_empty() && dq.status != "all"
    {
        parts.push(format!("status={}", urlencoding::encode(&dq.status)));
    }
    if !dq.payment.is_empty()
    {
        parts.push(format!("payment={}", urlencoding::encode(&dq.payment)));
    }
    if !dq.search.is_empty()
    {
        parts.push(format!("q={}", urlencoding::encode(&dq.search)));
    }
    if parts.is_empty()
    {
        "/admin".to_owned()
    }
    else
    {
        format!("/admin?{}", parts.join("&"))
    }
}

#[component]
fn FilterForm(query: DashboardQuery) -> impl IntoView
{
    let status = query.status.clone();
    let payment = query.payment.clone();
    let search = query.search.clone();
    let sort_col = query.sort_col.clone();
    let sort_dir = query.sort_dir.clone();

    let is_status = move |v: &str| status == v;
    let is_payment = move |v: &str| payment == v;

    view!
    {
        <form method="get" action="/admin" class="filters">
            <div class="filter-group">
                <label for="status-filter">"Statut"</label>
                <select id="status-filter" name="status">
                    <option value="all" selected=is_status("all") || query.status.is_empty()>"Tous"</option>
                    <option value="a_traiter" selected=is_status("a_traiter")>"A traiter"</option>
                    <option value="en_traitement" selected=is_status("en_traitement")>"En traitement"</option>
                    <option value="imprime" selected=is_status("imprime")>"Imprime"</option>
                    <option value="livre" selected=is_status("livre")>"Livre"</option>
                    <option value="annule" selected=is_status("annule")>"Annule"</option>
                </select>
            </div>
            <div class="filter-group">
                <label for="payment-filter">"Paiement"</label>
                <select id="payment-filter" name="payment">
                    <option value="" selected=query.payment.is_empty()>"Tous"</option>
                    <option value="gratuit" selected=is_payment("gratuit")>"Gratuit"</option>
                    <option value="requires" selected=is_payment("requires")>"Paiement requis"</option>
                </select>
            </div>
            <div class="filter-group">
                <label for="search-filter">"Recherche"</label>
                <input
                    id="search-filter"
                    name="q"
                    type="text"
                    placeholder="Nom du client ou #id..."
                    value=search
                />
            </div>
            // Preserve current sort state when applying filters
            <input type="hidden" name="sort" value=sort_col />
            <input type="hidden" name="dir" value=sort_dir />
            <div class="filter-group filter-actions">
                <label>"\u{00a0}"</label>
                <button type="submit" class="btn btn-primary">"Filtrer"</button>
            </div>
        </form>
    }
}