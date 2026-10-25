//! Bambu cloud MQTT client. Keeps one connection for all printers of the
//! account and maintains an in-memory view of their status and AMS content.

use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rumqttc::{
    AsyncClient, ConnectReturnCode, ConnectionError, Event, MqttOptions, Packet, QoS, Transport,
};
use serde::Deserialize;
use serde::de::Deserializer;
use tokio::sync::watch;

use crate::domain::filament::{
    BambuCredentials, ConnectionState, InventorySnapshot, LoadedSpool, PrinterDevice,
    PrinterSnapshot,
};
use crate::domain::repositories::PrinterTelemetry;

const BROKER_HOST: &str = "us.mqtt.bambulab.com";
const BROKER_PORT: u16 = 8883;
// Full reports already reach ~8 KB with two AMS, above rumqttc's 10 KB
// default once a print job adds its fields.
const MAX_PACKET_BYTES: usize = 1024 * 1024;
const MIN_BACKOFF: Duration = Duration::from_secs(2);
const MAX_BACKOFF: Duration = Duration::from_secs(120);
// Printers send a delta every few seconds while connected.
const FRESHNESS: Duration = Duration::from_secs(120);

pub struct BambuMqttMonitor {
    state: Arc<RwLock<MonitorState>>,
    session_tx: watch::Sender<Option<Session>>,
}

#[derive(Clone)]
struct Session {
    credentials: BambuCredentials,
    devices: Vec<PrinterDevice>,
}

struct MonitorState {
    connection: ConnectionState,
    printers: Vec<PrinterState>,
    ams_revision: u64,
}

struct PrinterState {
    snapshot: PrinterSnapshot,
    last_seen: Option<Instant>,
    parse_error_logged: bool,
}

impl PrinterState {
    fn new(device: PrinterDevice) -> Self {
        Self {
            snapshot: PrinterSnapshot::new(device),
            last_seen: None,
            parse_error_logged: false,
        }
    }

    fn mark_seen(&mut self) {
        self.last_seen = Some(Instant::now());
        self.snapshot.last_seen_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_secs());
    }
}

impl BambuMqttMonitor {
    /// Spawns the background connection task, idle until [`connect`] is
    /// called. Must be called from within a Tokio runtime.
    ///
    /// [`connect`]: PrinterTelemetry::connect
    pub fn start() -> Arc<Self> {
        let state = Arc::new(RwLock::new(MonitorState {
            connection: ConnectionState::NotConfigured,
            printers: Vec::new(),
            ams_revision: 0,
        }));
        let (session_tx, session_rx) = watch::channel(None);
        tokio::spawn(run(Arc::clone(&state), session_rx));
        Arc::new(Self { state, session_tx })
    }
}

impl PrinterTelemetry for BambuMqttMonitor {
    fn snapshot(&self) -> InventorySnapshot {
        let state = read(&self.state);
        let connected = state.connection == ConnectionState::Connected;
        InventorySnapshot {
            connection: state.connection.clone(),
            printers: state
                .printers
                .iter()
                .map(|printer| PrinterSnapshot {
                    fresh: connected
                        && printer
                            .last_seen
                            .is_some_and(|seen| seen.elapsed() < FRESHNESS),
                    ..printer.snapshot.clone()
                })
                .collect(),
            ams_revision: state.ams_revision,
        }
    }

    fn connect(&self, credentials: BambuCredentials, devices: Vec<PrinterDevice>) {
        {
            let mut state = write(&self.state);
            state.connection = ConnectionState::Connecting;
            state.printers = devices.iter().cloned().map(PrinterState::new).collect();
            state.ams_revision += 1;
        }
        self.session_tx.send_replace(Some(Session {
            credentials,
            devices,
        }));
    }
}

// A panic while holding the lock only leaves a stale status behind, which
// the next report overwrites, so poisoning is ignored.
fn read(state: &RwLock<MonitorState>) -> RwLockReadGuard<'_, MonitorState> {
    state
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn write(state: &RwLock<MonitorState>) -> RwLockWriteGuard<'_, MonitorState> {
    state
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn set_connection(state: &RwLock<MonitorState>, connection: ConnectionState) {
    write(state).connection = connection;
}

async fn run(state: Arc<RwLock<MonitorState>>, mut session_rx: watch::Receiver<Option<Session>>) {
    loop {
        let session = session_rx.borrow_and_update().clone();
        if let Some(session) = session {
            tokio::select! {
                () = run_session(&state, &session) => {
                    set_connection(&state, ConnectionState::TokenRejected);
                }
                changed = session_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    continue;
                }
            }
        }

        // Nothing configured, or the token was rejected: wait for the
        // admin to save a new one rather than hammering the broker.
        if session_rx.changed().await.is_err() {
            return;
        }
    }
}

/// Runs until the broker rejects the credentials. Any other error is
/// retried with an exponential backoff.
async fn run_session(state: &RwLock<MonitorState>, session: &Session) {
    let mut options = MqttOptions::new(
        format!("fablab-{}", uuid::Uuid::new_v4()),
        BROKER_HOST,
        BROKER_PORT,
    );
    options.set_credentials(
        format!("u_{}", session.credentials.user_id),
        session.credentials.access_token.clone(),
    );
    options.set_keep_alive(Duration::from_secs(30));
    options.set_max_packet_size(MAX_PACKET_BYTES, MAX_PACKET_BYTES);
    options.set_transport(Transport::tls_with_default_config());

    let (client, mut eventloop) = AsyncClient::new(options, 32);
    let mut backoff = MIN_BACKOFF;

    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::ConnAck(_))) => {
                backoff = MIN_BACKOFF;
                set_connection(state, ConnectionState::Connected);
                // Clean session: subscriptions do not survive a reconnect.
                for device in &session.devices {
                    request_full_report(&client, &device.serial);
                }
            }
            Ok(Event::Incoming(Packet::Publish(publish))) => {
                handle_report(state, &publish.topic, &publish.payload);
            }
            Ok(_) => {}
            Err(ConnectionError::ConnectionRefused(
                ConnectReturnCode::BadUserNamePassword | ConnectReturnCode::NotAuthorized,
            )) => {
                eprintln!("Bambu MQTT: token rejected by the broker");
                return;
            }
            Err(error) => {
                eprintln!("Bambu MQTT error: {error}");
                set_connection(state, ConnectionState::Unreachable(error.to_string()));
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
}

/// Subscribes to a printer and asks for its full state. Without the
/// request the printer only sends deltas, and the AMS block is only
/// included when it changes.
fn request_full_report(client: &AsyncClient, serial: &str) {
    // try_* never waits: the queue is drained by the poll loop we run in.
    if let Err(error) = client.try_subscribe(format!("device/{serial}/report"), QoS::AtMostOnce) {
        eprintln!("Bambu MQTT: subscribe failed: {error}");
    }
    let pushall = serde_json::json!({
        "pushing": { "sequence_id": "0", "command": "pushall" }
    });
    if let Err(error) = client.try_publish(
        format!("device/{serial}/request"),
        QoS::AtMostOnce,
        false,
        pushall.to_string(),
    ) {
        eprintln!("Bambu MQTT: pushall failed: {error}");
    }
}

#[derive(Deserialize)]
struct CloudReport {
    print: Option<PrintData>,
}

#[derive(Deserialize)]
struct PrintData {
    gcode_state: Option<String>,
    mc_percent: Option<u8>,
    mc_remaining_time: Option<u32>,
    layer_num: Option<u32>,
    total_layer_num: Option<u32>,
    nozzle_temper: Option<f32>,
    bed_temper: Option<f32>,
    subtask_name: Option<String>,
    ams: Option<AmsData>,
}

#[derive(Deserialize)]
struct AmsData {
    ams: Option<Vec<AmsUnit>>,
}

#[derive(Deserialize)]
struct AmsUnit {
    id: Option<String>,
    tray: Option<Vec<TrayData>>,
}

#[derive(Deserialize)]
struct TrayData {
    id: Option<String>,
    tray_type: Option<String>,
    tray_sub_brands: Option<String>,
    tray_info_idx: Option<String>,
    tray_color: Option<String>,
    // Lenient: Bambu sends -1 when the remaining amount is unknown, and
    // a strict u8 would make the whole report fail to deserialize.
    #[serde(default, deserialize_with = "de_opt_u8_from_num_or_str")]
    remain: Option<u8>,
    // A string ("1000") today; anything else is treated as unknown.
    tray_weight: Option<serde_json::Value>,
}

fn de_opt_u8_from_num_or_str<'de, D>(d: D) -> Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrStr {
        Num(i64),
        Str(String),
        // Any other shape (float, null, object) must not reject the report.
        Other(serde::de::IgnoredAny),
    }

    let v = Option::<NumOrStr>::deserialize(d)?;
    Ok(match v {
        None | Some(NumOrStr::Other(_)) => None,
        Some(NumOrStr::Num(n)) => u8::try_from(n).ok(),
        Some(NumOrStr::Str(s)) => s.parse::<u8>().ok(),
    })
}

fn handle_report(state: &RwLock<MonitorState>, topic: &str, payload: &[u8]) {
    let Some(serial) = topic
        .strip_prefix("device/")
        .and_then(|rest| rest.strip_suffix("/report"))
    else {
        return;
    };
    let report = serde_json::from_slice::<CloudReport>(payload);

    let mut state = write(state);
    let MonitorState {
        printers,
        ams_revision,
        ..
    } = &mut *state;
    let Some(printer) = printers
        .iter_mut()
        .find(|printer| printer.snapshot.device.serial == serial)
    else {
        return;
    };

    let report = match report {
        Ok(report) => report,
        Err(error) => {
            // Logged once per printer: deltas arrive every few seconds.
            if !printer.parse_error_logged {
                eprintln!(
                    "Bambu MQTT: unreadable report from {}: {error}",
                    printer.snapshot.device.name
                );
                printer.parse_error_logged = true;
            }
            return;
        }
    };
    printer.mark_seen();

    let Some(p) = report.print else {
        return;
    };
    let s = &mut printer.snapshot;
    if let Some(v) = p.gcode_state {
        s.gcode_state = v;
    }
    if let Some(v) = p.mc_percent {
        s.mc_percent = v;
    }
    if let Some(v) = p.mc_remaining_time {
        s.mc_remaining_time = v;
    }
    if let Some(v) = p.layer_num {
        s.layer_num = v;
    }
    if let Some(v) = p.total_layer_num {
        s.total_layer_num = v;
    }
    if let Some(v) = p.nozzle_temper {
        s.nozzle_temper = v;
    }
    if let Some(v) = p.bed_temper {
        s.bed_temper = v;
    }
    if let Some(v) = p.subtask_name {
        s.subtask_name = v;
    }
    // Delta reports often carry only `{"ams": {"version": N}}`;
    // keep the known slots unless the unit list is present.
    if let Some(units) = p.ams.and_then(|ams| ams.ams) {
        let spools = spools_from_units(units);
        if s.spools.as_ref() != Some(&spools) {
            s.spools = Some(spools);
            *ams_revision += 1;
        }
    }
}

fn spools_from_units(units: Vec<AmsUnit>) -> Vec<LoadedSpool> {
    units
        .into_iter()
        .flat_map(|unit| {
            let letter = ams_letter(unit.id.as_deref());
            unit.tray
                .unwrap_or_default()
                .into_iter()
                .filter_map(move |tray| tray_to_spool(letter, tray))
        })
        .collect()
}

/// AMS units are lettered A, B, C… on the printer screen.
fn ams_letter(id: Option<&str>) -> char {
    id.and_then(|id| id.parse::<u8>().ok())
        .filter(|index| *index < 26)
        .map_or('?', |index| char::from(b'A' + index))
}

/// Empty AMS slots only carry their id and state, so they are skipped.
fn tray_to_spool(ams_letter: char, tray: TrayData) -> Option<LoadedSpool> {
    let non_empty = |value: Option<String>| value.filter(|v| !v.is_empty());

    let filament_type = non_empty(tray.tray_type)?;
    let slot_number = tray.id?.parse::<u8>().ok()?.checked_add(1)?;

    Some(LoadedSpool {
        slot: format!("{ams_letter}{slot_number}"),
        product_id: non_empty(tray.tray_info_idx).unwrap_or_else(|| filament_type.clone()),
        name: non_empty(tray.tray_sub_brands).unwrap_or_else(|| filament_type.clone()),
        // RRGGBBAA from the AMS; anything else is dropped since the value
        // ends up in a CSS color.
        color_hex: tray
            .tray_color
            .and_then(|color| color.get(..6).map(str::to_uppercase))
            .filter(|color| color.chars().all(|c| c.is_ascii_hexdigit()))
            .unwrap_or_default(),
        remaining_percent: tray.remain,
        spool_weight_grams: match tray.tray_weight {
            Some(serde_json::Value::String(s)) => s.parse::<f64>().ok(),
            Some(serde_json::Value::Number(n)) => n.as_f64(),
            _ => None,
        }
        .filter(|weight| weight.is_finite() && *weight > 0.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spools_from(payload: &str) -> Vec<LoadedSpool> {
        let report: CloudReport = serde_json::from_str(payload).expect("report should parse");
        let units = report
            .print
            .and_then(|print| print.ams)
            .and_then(|ams| ams.ams)
            .unwrap_or_default();
        spools_from_units(units)
    }

    fn state_with_printer(serial: &str) -> RwLock<MonitorState> {
        RwLock::new(MonitorState {
            connection: ConnectionState::Connected,
            printers: vec![PrinterState::new(PrinterDevice {
                serial: serial.to_owned(),
                name: "X1C".to_owned(),
                model: "X1 Carbon".to_owned(),
            })],
            ams_revision: 0,
        })
    }

    const FULL_REPORT: &str = r#"{"print": {"command": "push_status", "gcode_state": "FINISH",
        "ams": {"ams": [
            {"id": "0", "tray": [
                {"id": "0", "state": 10},
                {"id": "1", "tray_type": "PETG", "tray_sub_brands": "PETG HF", "tray_info_idx": "GFG02",
                 "tray_color": "000000FF", "remain": 83, "tray_weight": "1000"}
            ]},
            {"id": "1", "tray": [
                {"id": "3", "tray_type": "PETG", "tray_sub_brands": "", "tray_info_idx": "GFG99",
                 "tray_color": "DCDCDCFF", "remain": -1, "tray_weight": "0"}
            ]}
        ]}}}"#;

    #[test]
    fn report_with_unknown_remain_still_parses() {
        let spools = spools_from(FULL_REPORT);

        assert_eq!(spools.len(), 2);
        assert_eq!(spools[0].slot, "A2");
        assert_eq!(spools[0].name, "PETG HF");
        assert_eq!(spools[0].material_key(), "GFG02:000000");
        assert_eq!(spools[0].remaining_grams(), Some(830.0));
        assert_eq!(spools[1].slot, "B4");
        assert_eq!(spools[1].name, "PETG");
        assert_eq!(spools[1].remaining_percent, None);
        assert_eq!(spools[1].spool_weight_grams, None);
    }

    #[test]
    fn remain_accepts_strings_and_rejects_out_of_range_values() {
        let spools = spools_from(
            r#"{"print": {"ams": {"ams": [{"id": "0", "tray": [
                {"id": "0", "tray_type": "PLA", "remain": "42"},
                {"id": "1", "tray_type": "PLA", "remain": 300},
                {"id": "2", "tray_type": "PLA", "remain": "-1"},
                {"id": "3", "tray_type": "PLA", "remain": 41.5}
            ]}]}}}"#,
        );

        let remains: Vec<_> = spools.iter().map(|spool| spool.remaining_percent).collect();
        assert_eq!(remains, vec![Some(42), None, None, None]);
    }

    #[test]
    fn delta_without_unit_list_keeps_the_known_spools() {
        let state = state_with_printer("SERIAL");
        handle_report(&state, "device/SERIAL/report", FULL_REPORT.as_bytes());
        handle_report(
            &state,
            "device/SERIAL/report",
            br#"{"print": {"ams": {"version": 228401}, "nozzle_temper": 24.0}}"#,
        );

        let state = read(&state);
        let printer = &state.printers[0].snapshot;
        assert_eq!(printer.spools.as_ref().map(Vec::len), Some(2));
        assert_eq!(printer.gcode_state, "FINISH");
        assert!((printer.nozzle_temper - 24.0).abs() < f32::EPSILON);
        assert_eq!(state.ams_revision, 1);
    }

    #[test]
    fn reports_from_unknown_printers_are_ignored() {
        let state = state_with_printer("SERIAL");
        handle_report(&state, "device/OTHER/report", FULL_REPORT.as_bytes());

        let state = read(&state);
        assert!(state.printers[0].snapshot.spools.is_none());
        assert!(state.printers[0].last_seen.is_none());
    }

    #[test]
    fn printers_are_fresh_only_while_connected() {
        let (session_tx, _session_rx) = watch::channel(None);
        let monitor = BambuMqttMonitor {
            state: Arc::new(state_with_printer("SERIAL")),
            session_tx,
        };
        handle_report(
            &monitor.state,
            "device/SERIAL/report",
            FULL_REPORT.as_bytes(),
        );
        assert!(monitor.snapshot().printers[0].fresh);

        set_connection(
            &monitor.state,
            ConnectionState::Unreachable("timeout".to_owned()),
        );
        assert!(!monitor.snapshot().printers[0].fresh);
    }
}
