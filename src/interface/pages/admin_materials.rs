use leptos::prelude::*;

use crate::application::dtos::order_output::MaterialView;

use crate::interface::components::bambu_banner::BambuStatusBanner;
use crate::interface::components::flash_message::FlashMessage;
use crate::interface::session_helpers::extract_admin_caller;

#[server]
pub async fn fetch_all_materials_with_stock() -> Result<Vec<MaterialView>, ServerFnError> {
    let _caller = extract_admin_caller().await?;

    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();
    let list_materials = state.list_materials.clone();
    tokio::task::spawn_blocking(move || {
        list_materials
            .execute_with_stock(false)
            .map_err(|e| ServerFnError::new(format!("{e}")))
    })
    .await
    .map_err(|e| ServerFnError::new(format!("{e}")))?
}

#[component]
pub fn AdminMaterialsPage() -> impl IntoView {
    let materials = Resource::new(|| (), |_| fetch_all_materials_with_stock());

    view! {
        <div class="page-card">
            <div class="page-header">
                <h1>"Materiaux"</h1>
                <a href="/admin/printers" class="btn">"Imprimantes"</a>
            </div>

            <FlashMessage />
            <BambuStatusBanner />

            <p class="hint">
                "Les materiaux sont crees automatiquement a partir des bobines chargees dans les AMS. "
                "Le disponible correspond au filament mesure par les imprimantes, moins le poids des commandes "
                "en attente ou en traitement."
            </p>

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
                                    <section class="empty-state">
                                        <p>"Aucun materiau : aucune bobine n'a encore ete detectee dans les AMS."</p>
                                    </section>
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
                                                <th>"Emplacement"</th>
                                                <th>"Mesure AMS"</th>
                                                <th>"Reserve"</th>
                                                <th>"Disponible"</th>
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
        </div>
    }
}

fn render_material_row(m: MaterialView) -> impl IntoView {
    let swatch = m
        .color_hex
        .filter(|hex| !hex.is_empty())
        .map(|hex| format!("background: #{hex}"));
    let location = if !m.available {
        "Non chargee".to_owned()
    } else if m.locations.is_empty() {
        "-".to_owned()
    } else {
        m.locations.join(", ")
    };
    let grams =
        |value: Option<f64>| value.map_or_else(|| "Inconnu".to_owned(), |g| format!("{g:.0} g"));
    let measured_str = grams(m.measured_remaining_grams);
    let reserved_str = format!("{:.0} g", m.reserved_grams);
    let available_str = grams(m.available_grams);

    let stock_class = match (m.available_grams, m.measured_remaining_grams) {
        (Some(available), _) if available < 0.0 => "stock-overdraft",
        (Some(available), Some(measured)) if available < measured * 0.10 => "stock-low",
        (Some(_), _) => "stock-ok",
        (None, _) => "stock-unknown",
    };

    view! {
        <tr>
            <td class="td-name">{m.name}</td>
            <td>
                {swatch.map(|style| view! { <span class="color-swatch" style=style></span> })}
                {m.color}
            </td>
            <td>{location}</td>
            <td>{measured_str}</td>
            <td>{reserved_str}</td>
            <td class=stock_class>{available_str}</td>
        </tr>
    }
}
