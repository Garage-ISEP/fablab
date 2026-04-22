use leptos::prelude::*;

use crate::application::dtos::order_output::OrderView;
use crate::application::dtos::order_sort::{OrderSort, SortColumn, SortDirection};
use super::status_badge::StatusBadge;

/// Renders the admin-or-student orders table.
///
/// The whole row is clickable: the `<a>` on the order id (which carries
/// the real href and stays focusable) is extended by a `::before`
/// pseudo-element covering the entire `<tr>`. This works in pure SSR
/// without JavaScript, keeps Cmd/Ctrl-click and middle-click behaviour
/// native, and remains accessible to screen readers because the anchor
/// itself is still the navigation target.
///
/// A wrapping `<div class="table-scroll ...">` applies the mobile
/// layout: admin tables stay tabular with horizontal scroll and a
/// sticky first column; client tables collapse into stacked cards
/// below 640px via CSS attribute labels.
#[component]
pub fn OrderTable(
    orders: Vec<OrderView>,
    is_admin: bool,
    #[prop(optional)] sort: Option<OrderSort>,
    /// Base URL with filters preserved, e.g. "/admin?status=a_traiter".
    /// Sort params will be appended. Required for sortable headers.
    #[prop(optional)] sort_base_url: Option<String>,
) -> impl IntoView
{
    let render_header = move |label: &'static str, col: SortColumn| -> AnyView
    {
        match (sort, sort_base_url.clone())
        {
            (Some(current), Some(base)) =>
            {
                let (next_dir, indicator) = if current.column == col
                {
                    let next = current.direction.toggled();
                    let arrow = match current.direction
                    {
                        SortDirection::Asc => " ^",
                        SortDirection::Desc => " v",
                    };
                    (next, arrow)
                }
                else
                {
                    (SortDirection::Asc, "")
                };

                let separator = if base.contains('?') { "&" } else { "?" };
                let dir_str = match next_dir
                {
                    SortDirection::Asc => "asc",
                    SortDirection::Desc => "desc",
                };
                let href = format!(
                    "{}{}sort={}&dir={}",
                    base, separator, col.as_str(), dir_str,
                );

                view!
                {
                    <th class="sortable">
                        <a href=href class="sort-link">
                            {label}{indicator}
                        </a>
                    </th>
                }
                .into_any()
            }
            _ =>
            {
                view! { <th>{label}</th> }.into_any()
            }
        }
    };

    let wrapper_class = if is_admin
    {
        "table-scroll table-scroll--admin"
    }
    else
    {
        "table-scroll table-scroll--client"
    };

    view!
    {
        <div class=wrapper_class>
            <table class="data-table">
                <thead>
                    <tr>
                        {render_header("#", SortColumn::Id)}
                        {render_header("Date", SortColumn::CreatedAt)}
                        {if is_admin
                        {
                            Some(render_header("Client", SortColumn::Client))
                        }
                        else
                        {
                            None
                        }}
                        {render_header("Materiau", SortColumn::Material)}
                        {render_header("Qte", SortColumn::Quantity)}
                        {render_header("Statut", SortColumn::Status)}
                        {render_header("Paiement", SortColumn::RequiresPayment)}
                        {if is_admin
                        {
                            let weight_h = render_header("Poids", SortColumn::Weight);
                            let time_h = render_header("Temps", SortColumn::PrintTime);
                            Some(view! { {weight_h} {time_h} })
                        }
                        else
                        {
                            None
                        }}
                    </tr>
                </thead>
                <tbody>
                    {orders.into_iter().map(|order|
                    {
                        let id = order.id;
                        let link = if is_admin
                        {
                            format!("/admin/order/{id}")
                        }
                        else
                        {
                            format!("/my-orders/{id}")
                        };
                        let aria_label = format!("Voir la commande {id}");
                        let status = order.status.clone();
                        let material = order.material_label.clone()
                            .unwrap_or_else(|| "-".to_owned());
                        let client_name = order.user_display_name.clone();
                        let date = order.created_at.chars().take(10).collect::<String>();
                        let req_label = if order.requires_payment { "Requis" } else { "Gratuit" };
                        let weight = order.sliced_weight_grams
                            .map(|w| format!("{w:.1} g"))
                            .unwrap_or_else(|| "-".to_owned());
                        let time = order.print_time_minutes
                            .map(|t| format!("{t} min"))
                            .unwrap_or_else(|| "-".to_owned());

                        view!
                        {
                            <tr class="clickable-row">
                                <td data-label="#">
                                    <a href=link class="row-link" aria-label=aria_label>{id}</a>
                                </td>
                                <td data-label="Date">{date}</td>
                                {if is_admin
                                {
                                    Some(view! { <td data-label="Client">{client_name}</td> })
                                }
                                else
                                {
                                    None
                                }}
                                <td data-label="Materiau">{material}</td>
                                <td data-label="Qte">{order.quantity}</td>
                                <td data-label="Statut"><StatusBadge status=status /></td>
                                <td data-label="Paiement">{req_label}</td>
                                {if is_admin
                                {
                                    Some(view!
                                    {
                                        <td data-label="Poids">{weight}</td>
                                        <td data-label="Temps">{time}</td>
                                    })
                                }
                                else
                                {
                                    None
                                }}
                            </tr>
                        }
                    }).collect::<Vec<_>>()}
                </tbody>
            </table>
        </div>
    }
}
