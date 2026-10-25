//! Filament inventory as reported by the Bambu printers' AMS.
//!
//! The printers are the single source of truth for how much filament is
//! left: each loaded spool reports a remaining percentage and its nominal
//! weight. Materials are keyed by product and color so that several spools
//! of the same product (on one or both printers) add up to one stock.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::material::LoadedMaterial;

/// Bambu cloud account used to read the printers. The access token grants
/// full control of the printers, so `Debug` never prints it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BambuCredentials {
    pub user_id: String,
    pub access_token: String,
}

impl fmt::Debug for BambuCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BambuCredentials")
            .field("user_id", &self.user_id)
            .field("access_token", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrinterDevice {
    pub serial: String,
    pub name: String,
    pub model: String,
}

/// Credentials and printer list saved by the admin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BambuAccount {
    pub credentials: BambuCredentials,
    pub devices: Vec<PrinterDevice>,
    /// When the token was saved (SQLite `datetime('now')`, UTC).
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadedSpool {
    /// Position as shown on the printer screen: AMS letter + slot, e.g. "B3".
    pub slot: String,
    /// Bambu filament id (`tray_info_idx`, e.g. "GFA00"), or the filament
    /// type for spools without one.
    pub product_id: String,
    pub name: String,
    /// `RRGGBB`, uppercase, without alpha.
    pub color_hex: String,
    /// `None` when the AMS cannot estimate it (non-RFID spool).
    pub remaining_percent: Option<u8>,
    /// Nominal net weight of a full spool, when the RFID tag provides it.
    pub spool_weight_grams: Option<f64>,
}

impl LoadedSpool {
    /// Identifies the product: same filament and same color means the same
    /// material, whatever printer or slot the spool sits in.
    pub fn material_key(&self) -> String {
        format!("{}:{}", self.product_id, self.color_hex)
    }

    pub fn remaining_grams(&self) -> Option<f64> {
        let percent = f64::from(self.remaining_percent?);
        let weight = self.spool_weight_grams.filter(|w| *w > 0.0)?;
        Some(weight * percent / 100.0)
    }

    pub fn to_loaded_material(&self) -> LoadedMaterial {
        LoadedMaterial {
            bambu_key: self.material_key(),
            name: self.name.clone(),
            color: color_name(&self.color_hex).to_owned(),
            color_hex: self.color_hex.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// No token saved yet.
    NotConfigured,
    Connecting,
    Connected,
    /// The broker refused the token: it expired or was revoked.
    TokenRejected,
    /// Network or broker error; the connection is retried automatically.
    Unreachable(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrinterSnapshot {
    pub device: PrinterDevice,
    /// True while the connection is up and the printer reported recently.
    /// Stock is only trusted from fresh printers.
    pub fresh: bool,
    pub last_seen_epoch: Option<u64>,
    pub gcode_state: String,
    pub mc_percent: u8,
    pub mc_remaining_time: u32,
    pub layer_num: u32,
    pub total_layer_num: u32,
    pub nozzle_temper: f32,
    pub bed_temper: f32,
    pub subtask_name: String,
    /// `None` until the printer has sent its AMS content at least once.
    pub spools: Option<Vec<LoadedSpool>>,
}

impl PrinterSnapshot {
    pub fn new(device: PrinterDevice) -> Self {
        Self {
            device,
            fresh: false,
            last_seen_epoch: None,
            gcode_state: String::new(),
            mc_percent: 0,
            mc_remaining_time: 0,
            layer_num: 0,
            total_layer_num: 0,
            nozzle_temper: 0.0,
            bed_temper: 0.0,
            subtask_name: String::new(),
            spools: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventorySnapshot {
    pub connection: ConnectionState,
    pub printers: Vec<PrinterSnapshot>,
    /// Bumped every time an AMS content changes, so callers can skip work
    /// when nothing moved.
    pub ams_revision: u64,
}

impl InventorySnapshot {
    /// Filament left for a material, summed over the spools of fresh
    /// printers that report a usable amount. `None` means the stock is
    /// unknown: no fresh printer holds a measurable spool of it.
    pub fn measured_remaining_grams(&self, material_key: &str) -> Option<f64> {
        self.fresh_spools()
            .filter(|spool| spool.material_key() == material_key)
            .filter_map(LoadedSpool::remaining_grams)
            .reduce(|total, grams| total + grams)
    }

    /// Products loaded in the AMS, deduplicated. `None` while some printer
    /// has not reported its AMS yet, so that a partial view never marks a
    /// loaded material as unavailable.
    pub fn loaded_materials(&self) -> Option<Vec<LoadedMaterial>> {
        let mut loaded: Vec<LoadedMaterial> = Vec::new();
        for printer in &self.printers {
            for spool in printer.spools.as_ref()? {
                let material = spool.to_loaded_material();
                if !loaded.iter().any(|m| m.bambu_key == material.bambu_key) {
                    loaded.push(material);
                }
            }
        }
        Some(loaded)
    }

    /// Where a material is loaded, e.g. "X1C B3 (9 %)", for the admin.
    pub fn locations(&self, material_key: &str) -> Vec<String> {
        self.printers
            .iter()
            .flat_map(|printer| {
                printer
                    .spools
                    .iter()
                    .flatten()
                    .filter(move |spool| spool.material_key() == material_key)
                    .map(move |spool| {
                        let remaining = spool
                            .remaining_percent
                            .map_or_else(|| "?".to_owned(), |p| format!("{p} %"));
                        format!("{} {} ({remaining})", printer.device.name, spool.slot)
                    })
            })
            .collect()
    }

    fn fresh_spools(&self) -> impl Iterator<Item = &LoadedSpool> {
        self.printers
            .iter()
            .filter(|printer| printer.fresh)
            .flat_map(|printer| printer.spools.iter().flatten())
    }
}

const NAMED_COLORS: &[(&str, (u8, u8, u8))] = &[
    ("Noir", (0x00, 0x00, 0x00)),
    ("Blanc", (0xFF, 0xFF, 0xFF)),
    ("Gris fonce", (0x40, 0x40, 0x40)),
    ("Gris", (0x80, 0x80, 0x80)),
    ("Gris clair", (0xC0, 0xC0, 0xC0)),
    ("Rouge", (0xC8, 0x20, 0x20)),
    ("Orange", (0xF0, 0x80, 0x20)),
    ("Jaune", (0xF0, 0xD0, 0x20)),
    ("Vert", (0x20, 0xA0, 0x40)),
    ("Vert clair", (0x90, 0xD0, 0x50)),
    ("Bleu", (0x20, 0x50, 0xC0)),
    ("Bleu clair", (0x60, 0xB0, 0xF0)),
    ("Violet", (0x80, 0x40, 0xB0)),
    ("Rose", (0xF0, 0x80, 0xB0)),
    ("Marron", (0x80, 0x40, 0x20)),
    ("Beige", (0xE0, 0xC8, 0xA0)),
];

/// Human color name for an AMS color, picked as the nearest entry of a
/// small palette. The AMS only reports a hex code.
pub fn color_name(hex: &str) -> &'static str {
    let channel = |range: std::ops::Range<usize>| {
        hex.get(range)
            .and_then(|s| u8::from_str_radix(s, 16).ok())
            .map(i32::from)
    };
    let (Some(r), Some(g), Some(b)) = (channel(0..2), channel(2..4), channel(4..6)) else {
        return "Inconnue";
    };

    NAMED_COLORS
        .iter()
        .min_by_key(|(_, (pr, pg, pb))| {
            let (dr, dg, db) = (r - i32::from(*pr), g - i32::from(*pg), b - i32::from(*pb));
            dr * dr + dg * dg + db * db
        })
        .map_or("Inconnue", |(name, _)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spool(product: &str, color: &str, percent: Option<u8>, weight: Option<f64>) -> LoadedSpool {
        LoadedSpool {
            slot: "A1".to_owned(),
            product_id: product.to_owned(),
            name: "PLA Basic".to_owned(),
            color_hex: color.to_owned(),
            remaining_percent: percent,
            spool_weight_grams: weight,
        }
    }

    fn printer(name: &str, fresh: bool, spools: Option<Vec<LoadedSpool>>) -> PrinterSnapshot {
        let mut snapshot = PrinterSnapshot::new(PrinterDevice {
            serial: format!("serial-{name}"),
            name: name.to_owned(),
            model: "P1S".to_owned(),
        });
        snapshot.fresh = fresh;
        snapshot.spools = spools;
        snapshot
    }

    fn inventory(printers: Vec<PrinterSnapshot>) -> InventorySnapshot {
        InventorySnapshot {
            connection: ConnectionState::Connected,
            printers,
            ams_revision: 1,
        }
    }

    #[test]
    fn remaining_grams_needs_percent_and_weight() {
        assert_eq!(
            spool("GFA00", "C12E1F", Some(41), Some(1000.0)).remaining_grams(),
            Some(410.0)
        );
        assert_eq!(
            spool("GFA00", "C12E1F", None, Some(1000.0)).remaining_grams(),
            None
        );
        assert_eq!(
            spool("GFL99", "C12E1F", Some(41), Some(0.0)).remaining_grams(),
            None
        );
    }

    #[test]
    fn measured_remaining_sums_spools_of_the_same_product_across_printers() {
        let snapshot = inventory(vec![
            printer(
                "P1S",
                true,
                Some(vec![spool("GFA00", "C12E1F", Some(100), Some(1000.0))]),
            ),
            printer(
                "X1C",
                true,
                Some(vec![spool("GFA00", "C12E1F", Some(9), Some(1000.0))]),
            ),
        ]);
        assert_eq!(
            snapshot.measured_remaining_grams("GFA00:C12E1F"),
            Some(1090.0)
        );
    }

    #[test]
    fn measured_remaining_ignores_stale_printers_and_unknown_spools() {
        let snapshot = inventory(vec![
            printer(
                "P1S",
                false,
                Some(vec![spool("GFA00", "C12E1F", Some(100), Some(1000.0))]),
            ),
            printer(
                "X1C",
                true,
                Some(vec![spool("GFA00", "C12E1F", None, Some(1000.0))]),
            ),
        ]);
        assert_eq!(snapshot.measured_remaining_grams("GFA00:C12E1F"), None);
    }

    #[test]
    fn loaded_materials_waits_for_every_printer() {
        let snapshot = inventory(vec![
            printer(
                "P1S",
                true,
                Some(vec![spool("GFA00", "C12E1F", Some(10), Some(1000.0))]),
            ),
            printer("X1C", true, None),
        ]);
        assert_eq!(snapshot.loaded_materials(), None);
    }

    #[test]
    fn loaded_materials_deduplicates_products() {
        let snapshot = inventory(vec![
            printer(
                "P1S",
                true,
                Some(vec![spool("GFA00", "C12E1F", Some(10), Some(1000.0))]),
            ),
            printer(
                "X1C",
                false,
                Some(vec![
                    spool("GFA00", "C12E1F", Some(90), Some(1000.0)),
                    spool("GFA00", "000000", Some(90), Some(1000.0)),
                ]),
            ),
        ]);
        let keys: Vec<String> = snapshot
            .loaded_materials()
            .unwrap()
            .into_iter()
            .map(|m| m.bambu_key)
            .collect();
        assert_eq!(keys, vec!["GFA00:C12E1F", "GFA00:000000"]);
    }

    #[test]
    fn color_name_picks_the_nearest_palette_entry() {
        assert_eq!(color_name("C12E1F"), "Rouge");
        assert_eq!(color_name("000000"), "Noir");
        assert_eq!(color_name("DCDCDC"), "Gris clair");
        assert_eq!(color_name("zz"), "Inconnue");
    }
}
