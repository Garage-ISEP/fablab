use leptos::prelude::*;
use leptos::server_fn::ServerFn;

use crate::application::use_cases::printers::PrinterOverview;
use crate::domain::filament::{ConnectionState, LoadedSpool, PrinterSnapshot, color_name};
use crate::interface::components::bambu_banner::BambuStatusBanner;
use crate::interface::components::csrf_field::CsrfField;
use crate::interface::components::flash_message::FlashMessage;
use crate::interface::session_helpers::extract_admin_caller;

/// Bambu does not document the token lifetime; OpenBambuAPI reports
/// about three months.
const TOKEN_LIFETIME_DAYS: i64 = 90;

#[server]
pub async fn fetch_printer_overview() -> Result<PrinterOverview, ServerFnError> {
    let caller = extract_admin_caller().await?;
    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();
    let printer_overview = state.printer_overview.clone();
    tokio::task::spawn_blocking(move || {
        printer_overview
            .execute(&caller)
            .map_err(|e| ServerFnError::new(format!("{e}")))
    })
    .await
    .map_err(|e| ServerFnError::new(format!("{e}")))?
}

#[server]
pub async fn set_bambu_token_action(token: String) -> Result<(), ServerFnError> {
    use crate::interface::error_messages::user_message;
    use crate::interface::flash::{FlashLevel, set_flash};
    use tower_sessions::Session;

    let caller = extract_admin_caller().await?;
    let session: Session = leptos_axum::extract().await?;

    let state = leptos::prelude::expect_context::<crate::interface::routes::AppState>();
    match state.configure_bambu.execute(&token, &caller).await {
        Ok(count) => {
            let message = format!("Token enregistre : {count} imprimante(s) trouvee(s).");
            let _ = set_flash(&session, FlashLevel::Success, &message).await;
        }
        Err(e) => {
            let _ = set_flash(&session, FlashLevel::Error, user_message(&e)).await;
        }
    }

    leptos_axum::redirect("/admin/printers");
    Ok(())
}

fn format_remaining_time(minutes: u32) -> String {
    let hours = minutes / 60;
    let minutes = minutes % 60;
    if hours > 0 {
        format!("{hours} h {minutes} min")
    } else {
        format!("{minutes} min")
    }
}

/// "2026-10-25 19:02:20" (UTC) -> ("25/10/2026", estimated expiry).
fn describe_token_date(saved_at: &str) -> Option<(String, String)> {
    use time::format_description::parse;

    let stored = parse("[year]-[month]-[day] [hour]:[minute]:[second]").ok()?;
    let display = parse("[day]/[month]/[year]").ok()?;
    let parsed = time::PrimitiveDateTime::parse(saved_at, &stored).ok()?;
    let expiry = parsed + time::Duration::days(TOKEN_LIFETIME_DAYS);
    Some((parsed.format(&display).ok()?, expiry.format(&display).ok()?))
}

fn connection_label(connection: &ConnectionState) -> String {
    match connection {
        ConnectionState::NotConfigured => "Non configuree".to_owned(),
        ConnectionState::Connecting => "Connexion en cours...".to_owned(),
        ConnectionState::Connected => "Connectee".to_owned(),
        ConnectionState::TokenRejected => "Token refuse ou expire".to_owned(),
        ConnectionState::Unreachable(reason) => format!("Injoignable ({reason})"),
    }
}

fn render_connection(
    connection: &ConnectionState,
    token_saved_at: Option<&str>,
) -> impl IntoView + use<> {
    let status = connection_label(connection);
    let token_info = match token_saved_at {
        Some(saved_at) => match describe_token_date(saved_at) {
            Some((saved, expiry)) => format!(
                "Token enregistre le {saved}. Il expire en general environ 3 mois apres sa creation (vers le {expiry})."
            ),
            None => format!("Token enregistre le {saved_at}."),
        },
        None => "Aucun token enregistre.".to_owned(),
    };

    view! {
        <section class="detail-card">
            <h2>"Connexion Bambu"</h2>
            <dl class="detail-list">
                <dt>"Etat"</dt><dd>{status}</dd>
                <dt>"Token"</dt><dd>{token_info}</dd>
            </dl>
            <form method="post" action=SetBambuTokenAction::url() class="material-form">
                <CsrfField />
                <div class="form-group">
                    <label for="bambu-token">"Nouveau token d'acces"</label>
                    <input
                        id="bambu-token"
                        name="token"
                        type="password"
                        autocomplete="off"
                        required
                        placeholder="Coller le token du compte de service"
                    />
                </div>
                <p class="hint">
                    "Le token s'obtient en se connectant au compte de service via l'API Bambu "
                    "(POST /v1/user-service/user/login, voir OpenBambuAPI). "
                    "Il est verifie aupres de Bambu avant d'etre enregistre, et la liste des imprimantes est recuperee automatiquement."
                </p>
                <button type="submit" class="btn btn-primary">"Enregistrer le token"</button>
            </form>
        </section>
    }
}

fn render_spool(spool: LoadedSpool) -> impl IntoView {
    let swatch = (!spool.color_hex.is_empty()).then(|| format!("background: #{}", spool.color_hex));
    let color = color_name(&spool.color_hex);
    let remaining = match (spool.remaining_percent, spool.remaining_grams()) {
        (Some(percent), Some(grams)) => format!("{percent} % (~{grams:.0} g)"),
        (Some(percent), None) => format!("{percent} %"),
        (None, _) => "Non disponible".to_owned(),
    };

    view! {
        <tr>
            <td class="td-name">{spool.slot}</td>
            <td>{spool.name}</td>
            <td>
                {swatch.map(|style| view! { <span class="color-swatch" style=style></span> })}
                {color}
            </td>
            <td>{remaining}</td>
        </tr>
    }
}

fn render_printer(printer: PrinterSnapshot) -> impl IntoView {
    let title = if printer.device.model.is_empty() || printer.device.model == printer.device.name {
        printer.device.name.clone()
    } else {
        format!("{} ({})", printer.device.name, printer.device.model)
    };
    let (status_class, status_label) = if printer.fresh {
        ("printer-online", "En ligne")
    } else {
        ("printer-offline", "Hors ligne : niveaux non garantis")
    };

    view! {
        <section class="printer-status">
            <h2>{title}</h2>
            <p class="hint">{format!("Numero de serie : {}", printer.device.serial)}</p>
            <p class=status_class>{status_label}</p>
            <table class="data-table">
                <tbody>
                    <tr>
                        <td class="td-name">"Etat"</td>
                        <td>{printer.gcode_state}</td>
                    </tr>
                    <tr>
                        <td class="td-name">"Progression"</td>
                        <td>{format!("{} %", printer.mc_percent)}</td>
                    </tr>
                    <tr>
                        <td class="td-name">"Temps restant"</td>
                        <td>{format_remaining_time(printer.mc_remaining_time)}</td>
                    </tr>
                    <tr>
                        <td class="td-name">"Couche"</td>
                        <td>{format!("{} / {}", printer.layer_num, printer.total_layer_num)}</td>
                    </tr>
                    <tr>
                        <td class="td-name">"Fichier"</td>
                        <td>{printer.subtask_name}</td>
                    </tr>
                    <tr>
                        <td class="td-name">"Buse"</td>
                        <td>{format!("{:.1} °C", printer.nozzle_temper)}</td>
                    </tr>
                    <tr>
                        <td class="td-name">"Plateau"</td>
                        <td>{format!("{:.1} °C", printer.bed_temper)}</td>
                    </tr>
                </tbody>
            </table>
            <h3>"Filament (AMS)"</h3>
            {match printer.spools
            {
                None => view! { <p class="hint">"Contenu de l'AMS pas encore recu."</p> }.into_any(),
                Some(spools) if spools.is_empty() => view! { <p>"Aucune bobine chargee."</p> }.into_any(),
                Some(spools) => view!
                {
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th>"Slot"</th>
                                <th>"Filament"</th>
                                <th>"Couleur"</th>
                                <th>"Restant"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {spools.into_iter().map(render_spool).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                }.into_any(),
            }}
        </section>
    }
}

#[component]
pub fn AdminPrintersPage() -> impl IntoView {
    let overview = Resource::new(|| (), |_| fetch_printer_overview());

    view! {
        <div class="page-card">
            <div class="page-header">
                <h1>"Imprimantes"</h1>
                <a href="/admin/printers" class="btn">"Rafraichir"</a>
            </div>

            <FlashMessage />
            <BambuStatusBanner />

            <Suspense fallback=|| view! { <p class="loading">"Chargement..."</p> }>
                {move || overview.get().map(|result| match result {
                    Ok(overview) => {
                        let connection = render_connection(
                            &overview.inventory.connection,
                            overview.token_saved_at.as_deref(),
                        );
                        let printers = overview.inventory.printers;
                        view! {
                            {connection}
                            {printers.into_iter().map(render_printer).collect::<Vec<_>>()}
                        }.into_any()
                    }
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
