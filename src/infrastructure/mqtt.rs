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
    pub remaining_percent: u8,
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
    remain: Option<u8>,
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
                        if let Some(ams) = p.ams {
                            s.filament_slots = ams
                                .ams
                                .unwrap_or_default()
                                .into_iter()
                                .flat_map(|unit| unit.tray.unwrap_or_default())
                                .filter_map(|tray| {
                                    Some(FilamentSlot {
                                        slot: tray.id?,
                                        filament_type: tray
                                            .tray_type
                                            .or(tray.tray_sub_brands)
                                            .unwrap_or_default(),
                                        color: tray.tray_color.unwrap_or_default(),
                                        remaining_percent: tray.remain?,
                                    })
                                })
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
