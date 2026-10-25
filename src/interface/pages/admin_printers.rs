use leptos::prelude::*;

use leptos::{IntoView, view};
use serde::{Deserialize, Serialize};

use crate::infrastructure::mqtt::PrinterStatus;
use crate::interface::session_helpers::extract_admin_caller;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrinterView {
    pub device_serial: String,
    pub status: PrinterStatus,
}

#[server]
pub async fn fetch_printer_info() -> Result<Vec<PrinterView>, ServerFnError> {
    let _caller = extract_admin_caller().await?;
    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();
    let statuses = state.printer_statuses.read().await;

    Ok(statuses
        .iter()
        .map(|(device_serial, status)| PrinterView {
            device_serial: device_serial.clone(),
            status: status.clone(),
        })
        .collect())
}

fn format_remaining_time(seconds: u32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{hours} h {minutes} min")
    } else {
        format!("{minutes} min")
    }
}

fn render_printer(printer: PrinterView) -> impl IntoView {
    let status = printer.status;
    view! {
        <section class="printer-status">
            <h2>{format!("Imprimante {}", printer.device_serial)}</h2>
            <p class=if status.online { "printer-online" } else { "printer-offline" }>
                {if status.online { "En ligne" } else { "Hors ligne" }}
            </p>
            {if status.online {
                view! {
                    <table class="data-table">
                        <tbody>
                            <tr>
                                <td class="td-name">"Etat"</td>
                                <td>{status.gcode_state}</td>
                            </tr>
                            <tr>
                                <td class="td-name">"Progression"</td>
                                <td>{format!("{} %", status.mc_percent)}</td>
                            </tr>
                            <tr>
                                <td class="td-name">"Temps restant"</td>
                                <td>{format_remaining_time(status.mc_remaining_time)}</td>
                            </tr>
                            <tr>
                                <td class="td-name">"Couche"</td>
                                <td>{format!("{} / {}", status.layer_num, status.total_layer_num)}</td>
                            </tr>
                            <tr>
                                <td class="td-name">"Fichier"</td>
                                <td>{status.subtask_name}</td>
                            </tr>
                            <tr>
                                <td class="td-name">"Buse"</td>
                                <td>{format!("{:.1} °C", status.nozzle_temper)}</td>
                            </tr>
                            <tr>
                                <td class="td-name">"Plateau"</td>
                                <td>{format!("{:.1} °C", status.bed_temper)}</td>
                            </tr>
                        </tbody>
                    </table>
                    <h3>"Filament"</h3>
                    {if !status.filament_slots.is_empty() {
                        view! {
                            <table class="data-table">
                                <thead>
                                    <tr>
                                        <th>"Slot"</th>
                                        <th>"Type"</th>
                                        <th>"Couleur"</th>
                                        <th>"Restant"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {status.filament_slots.into_iter().map(|filament| view! {
                                        <tr>
                                            <td>{filament.slot}</td>
                                            <td>{filament.filament_type}</td>
                                            <td>{filament.color}</td>
                                            <td>{format!("{} %", filament.remaining_percent)}</td>
                                        </tr>
                                    }).collect::<Vec<_>>()}
                                </tbody>
                            </table>
                        }.into_any()
                    } else {
                        view! { <p>"Aucun filament detecte."</p> }.into_any()
                    }}
                }.into_any()
            } else {
                view! { <p>"Aucune donnee disponible hors ligne."</p> }.into_any()
            }}
        </section>
    }
}

#[component]
pub fn AdminPrintersPage() -> impl IntoView {
    let printers = Resource::new(|| (), |_| fetch_printer_info());

    view! {
        <div class="page-card">
            <div class="page-header">
                <h1>"Imprimantes"</h1>
            </div>

            <Suspense fallback=|| view! { <p class="loading">"Chargement..."</p> }>
                {move || printers.get().map(|result| match result {
                    Ok(printers) if printers.is_empty() => view! {
                        <section class="empty-state">
                            <p>"Aucune imprimante configuree."</p>
                        </section>
                    }.into_any(),
                    Ok(printers) => view! {
                        {printers.into_iter().map(render_printer).collect::<Vec<_>>()}
                    }.into_any(),
                    Err(_) => view! {
                        <div class="alert alert-error">
                            <p>"Acces refuse."</p>
                            <a href="/admin/login" class="btn btn-primary">"Se connecter"</a>
                        </div>
                    }.into_any(),
                })}
            </Suspense>
        </div>
    }
}
