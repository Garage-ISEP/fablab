use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

use rumqttc::{AsyncClient, MqttOptions, QoS, Transport};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PrinterStatus {
    pub online: bool,
    pub last_seen_epoch: Option<u64>,
    pub gcode_state: String,
    pub mc_percent: u8,
    pub mc_remaining_time: u32,
    pub layer_num: u32,
    pub total_layer_num: u32,
    pub nozzle_temper: f32,
    pub bed_temper: f32,
    pub subtask_name: String,
    pub filament_slots: Vec<FilamentSlot>,
}

impl PrinterStatus {
    fn mark_online(&mut self) {
        self.online = true;
        self.last_seen_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_secs());
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FilamentSlot {
    pub slot: String,
    pub filament_type: String,
    pub color: String,
    /// `None` when the printer cannot estimate it (Bambu reports `-1`).
    pub remaining_percent: Option<u8>,
}

pub type SharedPrinterStatuses = Arc<RwLock<HashMap<String, PrinterStatus>>>;

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
    tray: Option<Vec<TrayData>>,
}

#[derive(Deserialize)]
struct TrayData {
    id: Option<String>,
    tray_type: Option<String>,
    tray_sub_brands: Option<String>,
    tray_color: Option<String>,
    // Signed: Bambu sends -1 when the remaining amount is unknown, which
    // would make the whole report fail to deserialize as u8.
    remain: Option<i16>,
}

/// Empty AMS slots only carry their id, so they are skipped.
fn tray_to_slot(tray: TrayData) -> Option<FilamentSlot> {
    let filament_type = tray
        .tray_type
        .or(tray.tray_sub_brands)
        .filter(|filament_type| !filament_type.is_empty())?;

    Some(FilamentSlot {
        slot: tray.id?,
        filament_type,
        color: tray.tray_color.unwrap_or_default(),
        remaining_percent: tray.remain.and_then(|remain| u8::try_from(remain).ok()),
    })
}

pub async fn start_cloud_mqtt_listener(
    uid: String,
    token: String,
    device_serial: String,
    statuses: SharedPrinterStatuses,
) -> Result<(), Box<dyn std::error::Error>> {
    statuses
        .write()
        .await
        .entry(device_serial.clone())
        .or_default();

    let mut mqttoptions = MqttOptions::new(
        format!("fablab-{}", uuid::Uuid::new_v4()),
        "us.mqtt.bambulab.com",
        8883,
    );

    mqttoptions.set_credentials(format!("u_{}", uid), token);
    mqttoptions.set_keep_alive(Duration::from_secs(60));
    mqttoptions.set_transport(Transport::tls_with_default_config());

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    let topic = format!("device/{}/report", device_serial);
    client.subscribe(topic, QoS::AtMostOnce).await?;

    // Without this request the printer only sends deltas, and the AMS
    // block is only included when it changes.
    let pushall = serde_json::json!({
        "pushing": { "sequence_id": "0", "command": "pushall" }
    });
    client
        .publish(
            format!("device/{}/request", device_serial),
            QoS::AtMostOnce,
            false,
            pushall.to_string(),
        )
        .await?;

    loop {
        match eventloop.poll().await {
            Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(publish))) => {
                let payload = String::from_utf8_lossy(&publish.payload);
                let mut statuses = statuses.write().await;
                let s = statuses.entry(device_serial.clone()).or_default();
                s.mark_online();

                if let Ok(report) = serde_json::from_str::<CloudReport>(&payload) {
                    if let Some(p) = report.print {
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
                            s.filament_slots = units
                                .into_iter()
                                .flat_map(|unit| unit.tray.unwrap_or_default())
                                .filter_map(tray_to_slot)
                                .collect();
                        }
                    }
                }
            }
            Err(e) => {
                statuses
                    .write()
                    .await
                    .insert(device_serial.clone(), PrinterStatus::default());
                eprintln!("MQTT error: {:?}", e);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slots_from(payload: &str) -> Vec<FilamentSlot> {
        let report: CloudReport = serde_json::from_str(payload).expect("report should parse");
        report
            .print
            .and_then(|print| print.ams)
            .and_then(|ams| ams.ams)
            .unwrap_or_default()
            .into_iter()
            .flat_map(|unit| unit.tray.unwrap_or_default())
            .filter_map(tray_to_slot)
            .collect()
    }

    #[test]
    fn report_with_unknown_remain_still_parses() {
        let slots = slots_from(
            r#"{"print": {"gcode_state": "IDLE", "ams": {"ams": [{"id": "0", "tray": [
                {"id": "0", "tray_type": "PLA", "tray_color": "FFFFFFFF", "remain": 87},
                {"id": "1", "tray_type": "PETG", "tray_color": "000000FF", "remain": -1},
                {"id": "2"},
                {"id": "3", "tray_type": "", "remain": -1}
            ]}]}}}"#,
        );

        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].slot, "0");
        assert_eq!(slots[0].remaining_percent, Some(87));
        assert_eq!(slots[1].filament_type, "PETG");
        assert_eq!(slots[1].remaining_percent, None);
    }
}
