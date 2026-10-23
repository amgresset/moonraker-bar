//! Data model shared between the Rust backend and the web front.
//!
//! Everything here is serialised with `serde` and mirrored in `src/lib/types.ts`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A printer as configured by the user (persisted in the Tauri store).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrinterConfig {
    pub id: String,
    pub name: String,
    /// Moonraker HTTP base URL, e.g. `http://voron.local:7125`.
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Snapshot URL for the camera. Absolute, or relative to `url`
    /// (e.g. `/webcam/?action=snapshot`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_url: Option<String>,
    /// Web UI (Mainsail / Fluidd). Empty = the Moonraker host without its port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
    /// Klipper macros shown as buttons in the panel, in display order.
    #[serde(default)]
    pub pinned_macros: Vec<String>,
    /// Temperature objects shown as tiles, in display order. Empty = sensible default.
    #[serde(default)]
    pub shown_temps: Vec<String>,
    /// Illustration to draw: `voron24`, `trident`, `v0`, `bedslinger`, `generic`. None = detect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl PrinterConfig {
    /// Websocket endpoint derived from the HTTP base URL.
    pub fn websocket_url(&self) -> Result<url::Url, String> {
        let mut u = url::Url::parse(self.url.trim()).map_err(|e| format!("Invalid URL: {e}"))?;
        let scheme = match u.scheme() {
            "http" => "ws",
            "https" => "wss",
            "ws" | "wss" => u.scheme(),
            other => return Err(format!("Unsupported scheme: {other}")),
        }
        .to_string();
        u.set_scheme(&scheme).map_err(|_| "Unsupported scheme".to_string())?;
        u.set_path("/websocket");
        u.set_query(None);
        Ok(u)
    }

    /// Absolute camera snapshot URL, if any. (The front resolves it too; kept for
    /// future backend use such as thumbnails in notifications.)
    #[allow(dead_code)]
    pub fn resolved_camera_url(&self) -> Option<String> {
        let raw = self.camera_url.as_deref()?.trim();
        if raw.is_empty() {
            return None;
        }
        if raw.starts_with("http://") || raw.starts_with("https://") {
            return Some(raw.to_string());
        }
        let base = url::Url::parse(self.url.trim()).ok()?;
        base.join(raw).ok().map(|u| u.to_string())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    #[default]
    Offline,
    Connecting,
    Connected,
}

/// A `[gcode_macro NAME]` section discovered on the printer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MacroInfo {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// One target inside a preheat preset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresetTarget {
    /// Klipper object, e.g. `extruder`, `heater_bed`, `heater_generic chamber`, `temperature_fan hotend`.
    pub object: String,
    /// `heater` or `temperature_fan`.
    pub kind: String,
    pub value: f64,
}

/// A preheat preset read from Mainsail's or Fluidd's settings in the Moonraker database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Preset {
    pub id: String,
    pub name: String,
    /// `mainsail` or `fluidd`.
    pub source: String,
    pub targets: Vec<PresetTarget>,
    #[serde(default)]
    pub gcode: String,
}

impl Preset {
    /// The script Mainsail would send for this preset.
    pub fn to_gcode(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for t in &self.targets {
            match t.kind.as_str() {
                "temperature_fan" => {
                    let name = t.object.strip_prefix("temperature_fan ").unwrap_or(&t.object);
                    lines.push(format!("SET_TEMPERATURE_FAN_TARGET TEMPERATURE_FAN={name} TARGET={}", t.value));
                }
                _ => {
                    let name = t.object.strip_prefix("heater_generic ").unwrap_or(&t.object);
                    lines.push(format!("SET_HEATER_TEMPERATURE HEATER={name} TARGET={}", t.value));
                }
            }
        }
        let extra = self.gcode.trim();
        if !extra.is_empty() {
            lines.push(extra.to_string());
        }
        lines.join("\n")
    }
}

/// What the Klipper config says about the machine, used to pick its illustration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Machine {
    /// `corexy`, `cartesian`, `corexz`, `delta`…
    pub kinematics: String,
    pub x_max: f64,
    pub y_max: f64,
    pub z_max: f64,
    /// `qgl` (quad_gantry_level), `z_tilt`, or empty.
    pub leveling: String,
}

/// A webcam as configured in Moonraker (`[webcam]` sections, or added from Mainsail / Fluidd).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Webcam {
    pub name: String,
    pub enabled: bool,
    /// `mjpegstreamer`, `mjpegstreamer-adaptive`, `webrtc-camerastreamer`, `hlsstream`, `ipstream`…
    pub service: String,
    /// Absolute or relative to the web UI host, e.g. `/webcam/?action=stream`.
    pub stream_url: String,
    pub snapshot_url: String,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    /// 0, 90, 180 or 270.
    pub rotation: i64,
}

/// One heater, sensor or temperature fan, as discovered on the printer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TempReading {
    /// Klipper object: `extruder`, `heater_bed`, `heater_generic chamber`, `temperature_sensor mcu`…
    pub object: String,
    /// `extruder` | `heater_bed` | `heater_generic` | `temperature_sensor` | `temperature_fan`.
    pub kind: String,
    pub temperature: f64,
    /// Heaters and temperature fans only.
    pub target: Option<f64>,
    /// Heaters only, 0..1.
    pub power: Option<f64>,
    /// Temperature fans only, 0..1.
    pub speed: Option<f64>,
}

impl TempReading {
    pub fn is_temperature_object(object: &str) -> Option<&'static str> {
        if object == "extruder" || (object.starts_with("extruder") && object[8..].chars().all(|c| c.is_ascii_digit())) {
            Some("extruder")
        } else if object == "heater_bed" {
            Some("heater_bed")
        } else if object.starts_with("heater_generic ") {
            Some("heater_generic")
        } else if object.starts_with("temperature_sensor ") {
            Some("temperature_sensor")
        } else if object.starts_with("temperature_fan ") {
            Some("temperature_fan")
        } else {
            None
        }
    }

    /// Fields to subscribe for this kind of object.
    pub fn fields(kind: &str) -> Value {
        match kind {
            "temperature_sensor" => json!(["temperature"]),
            "temperature_fan" => json!(["temperature", "target", "speed"]),
            _ => json!(["temperature", "target", "power"]),
        }
    }
}

// ---------- AFC (Armored Turtle Automated Filament Control) ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AfcLane {
    /// Klipper object name, e.g. `AFC_lane lane1`.
    pub object: String,
    pub name: String,
    pub unit: String,
    pub hub: String,
    pub extruder: String,
    /// Position inside its unit (1-based).
    pub index: i64,
    /// Tool macro, e.g. `T0`.
    pub map: String,
    /// Filament at the load sensor / at the prep sensor.
    pub load: bool,
    pub prep: bool,
    pub tool_loaded: bool,
    pub loaded_to_hub: bool,
    pub material: String,
    /// `#RRGGBB` or empty.
    pub color: String,
    pub filament_name: String,
    /// Remaining weight, grams.
    pub weight: f64,
    pub spool_id: Option<i64>,
    /// `In Tool`, `Ready`, `Prep`, `Not Ready`.
    pub filament_status: String,
    /// Lane state machine: `None`, `Loaded`, `Tooled`, `Tool Loading`, `Ejecting`, `Error`…
    pub status: String,
    pub runout_lane: String,
    pub extruder_temp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AfcHub {
    pub object: String,
    pub name: String,
    /// Filament present in the hub.
    pub state: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AfcExtruder {
    pub object: String,
    pub name: String,
    pub lane_loaded: String,
    pub tool_start_status: Option<bool>,
    pub tool_end_status: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AfcState {
    /// Lane currently loaded in the toolhead.
    pub current_load: Option<String>,
    /// Lane being loaded / unloaded right now.
    pub current_lane: Option<String>,
    pub next_lane: Option<String>,
    /// `Idle`, `Loading`, `Unloading`, `ToolSwap`, `Ejecting`, `Error`…
    pub current_state: String,
    pub error_state: bool,
    pub bypass_state: bool,
    pub current_toolchange: i64,
    pub number_of_toolchanges: i64,
    pub spoolman: bool,
    pub message: String,
    pub message_type: String,
    /// `"<type> <name>"` per unit, e.g. `Box_Turtle Turtle_1`.
    pub units: Vec<String>,
    pub lanes: Vec<AfcLane>,
    pub hubs: Vec<AfcHub>,
    pub extruders: Vec<AfcExtruder>,
}

/// Object names discovered for AFC, used to route status deltas.
#[derive(Debug, Clone, Default)]
pub struct AfcKeys {
    pub lanes: Vec<String>,
    pub hubs: Vec<String>,
    pub extruders: Vec<String>,
}

impl AfcKeys {
    pub fn from_objects(objects: &[String]) -> Option<Self> {
        if !objects.iter().any(|o| o == "AFC") {
            return None;
        }
        let mut keys = Self::default();
        for o in objects {
            if o.starts_with("AFC_lane ") || o.starts_with("AFC_stepper ") {
                keys.lanes.push(o.clone());
            } else if o.starts_with("AFC_hub ") {
                keys.hubs.push(o.clone());
            } else if o.starts_with("AFC_extruder ") {
                keys.extruders.push(o.clone());
            }
        }
        Some(keys)
    }

    pub fn subscription(&self, objects: &mut Value) {
        objects["AFC"] = Value::Null;
        for k in &self.lanes {
            objects[k] = Value::Null;
        }
        for k in &self.hubs {
            objects[k] = json!(["state", "lanes"]);
        }
        for k in &self.extruders {
            objects[k] = json!(["lane_loaded", "tool_start_status", "tool_end_status", "lanes"]);
        }
    }
}

fn short_name(object: &str) -> String {
    object.split_once(' ').map(|(_, n)| n).unwrap_or(object).to_string()
}

impl AfcState {
    fn apply(&mut self, status: &Value, keys: &AfcKeys) {
        if let Some(a) = status.get("AFC") {
            set_opt_str(&mut self.current_load, a.get("current_load"));
            set_opt_str(&mut self.current_lane, a.get("current_lane"));
            set_opt_str(&mut self.next_lane, a.get("next_lane"));
            set_str(&mut self.current_state, a.get("current_state"));
            set_bool(&mut self.error_state, a.get("error_state"));
            set_bool(&mut self.bypass_state, a.get("bypass_state"));
            set_i64(&mut self.current_toolchange, a.get("current_toolchange"));
            set_i64(&mut self.number_of_toolchanges, a.get("number_of_toolchanges"));
            set_bool(&mut self.spoolman, a.get("spoolman"));
            if let Some(units) = a.get("units").and_then(Value::as_array) {
                self.units = units.iter().filter_map(Value::as_str).map(str::to_string).collect();
            }
            if let Some(m) = a.get("message") {
                match m {
                    Value::Object(o) => {
                        self.message = o.get("message").and_then(Value::as_str).unwrap_or("").to_string();
                        self.message_type = o.get("type").and_then(Value::as_str).unwrap_or("").to_string();
                    }
                    Value::String(s) => self.message = s.clone(),
                    _ => {
                        self.message.clear();
                        self.message_type.clear();
                    }
                }
            }
        }

        for key in &keys.lanes {
            let Some(v) = status.get(key) else { continue };
            let lane = match self.lanes.iter_mut().position(|l| &l.object == key) {
                Some(i) => &mut self.lanes[i],
                None => {
                    self.lanes.push(AfcLane { object: key.clone(), name: short_name(key), ..Default::default() });
                    self.lanes.last_mut().unwrap()
                }
            };
            set_str(&mut lane.name, v.get("name"));
            set_str(&mut lane.unit, v.get("unit"));
            set_str(&mut lane.hub, v.get("hub"));
            set_str(&mut lane.extruder, v.get("extruder"));
            set_i64(&mut lane.index, v.get("lane"));
            set_str(&mut lane.map, v.get("map"));
            set_bool(&mut lane.load, v.get("load"));
            set_bool(&mut lane.prep, v.get("prep"));
            set_bool(&mut lane.tool_loaded, v.get("tool_loaded"));
            set_bool(&mut lane.loaded_to_hub, v.get("loaded_to_hub"));
            set_str(&mut lane.material, v.get("material"));
            if let Some(c) = v.get("color").and_then(Value::as_str) {
                let c = c.trim();
                lane.color = if c.is_empty() || c.starts_with('#') { c.to_string() } else { format!("#{c}") };
            }
            set_str(&mut lane.filament_name, v.get("filament_name"));
            set_f64(&mut lane.weight, v.get("weight"));
            if let Some(s) = v.get("spool_id") {
                lane.spool_id = s.as_i64();
            }
            set_str(&mut lane.filament_status, v.get("filament_status"));
            set_str(&mut lane.status, v.get("status"));
            set_str(&mut lane.runout_lane, v.get("runout_lane"));
            set_f64(&mut lane.extruder_temp, v.get("extruder_temp"));
        }
        self.lanes.sort_by(|a, b| a.unit.cmp(&b.unit).then(a.index.cmp(&b.index)).then(a.name.cmp(&b.name)));

        for key in &keys.hubs {
            let Some(v) = status.get(key) else { continue };
            let hub = match self.hubs.iter_mut().position(|h| &h.object == key) {
                Some(i) => &mut self.hubs[i],
                None => {
                    self.hubs.push(AfcHub { object: key.clone(), name: short_name(key), ..Default::default() });
                    self.hubs.last_mut().unwrap()
                }
            };
            set_bool(&mut hub.state, v.get("state"));
        }

        for key in &keys.extruders {
            let Some(v) = status.get(key) else { continue };
            let ex = match self.extruders.iter_mut().position(|e| &e.object == key) {
                Some(i) => &mut self.extruders[i],
                None => {
                    self.extruders.push(AfcExtruder { object: key.clone(), name: short_name(key), ..Default::default() });
                    self.extruders.last_mut().unwrap()
                }
            };
            if let Some(l) = v.get("lane_loaded") {
                ex.lane_loaded = l.as_str().unwrap_or("").to_string();
            }
            if let Some(b) = v.get("tool_start_status") {
                ex.tool_start_status = b.as_bool();
            }
            if let Some(b) = v.get("tool_end_status") {
                ex.tool_end_status = b.as_bool();
            }
        }
    }
}

// ---------- MultiACE (ACE multi-material units) ----------
//
// Field and command names follow `multiace/docs/ENGINE_API.md` (api_version 1):
// only the fields documented there are relied on. Every index on the wire is
// 0-based; multiACE's own web UI and logs offset them by `[ace]
// display_index_base` (shipped as 1), so the slot this card calls S0 reads
// "Slot 1" there.

/// Fields of the `ace` object the card needs. The object also carries the
/// persisted settings, the spool table history and a calibration session;
/// subscribing to a list keeps the deltas small.
const ACE_FIELDS: &[&str] = &[
    "api_version",
    "status",
    "mode",
    "device_count",
    "active_device",
    "temp",
    "dryer_status",
    "aces",
    "spools",
    "spool_binding",
    "head_source",
    "head_manual",
    "head_feeder",
    "head_ace",
    "swap_phase",
    "swap_in_progress",
    "unload_all_active",
    "last_swap_result",
    "calibration",
    "spoolman_auto",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AceDryer {
    /// `stop`, `drying`, `heating`…
    pub status: String,
    pub target_temp: f64,
    /// Whole run, minutes.
    pub duration: i64,
    /// Minutes left.
    pub remain_time: i64,
}

/// One ACE unit, from `ace.aces[]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AceUnit {
    /// Device index, 0-based (`ACE=` / `TARGET=`).
    pub index: i64,
    pub connected: bool,
    pub temp: f64,
    /// Relative humidity in percent; only an ACE 2 measures its own.
    pub humidity: Option<f64>,
    pub dryer: AceDryer,
}

/// One slot of an ACE unit: the unit's own report, enriched from the spool table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AceSlot {
    /// Unit index, 0-based (`ACE=`).
    pub ace: i64,
    /// Slot inside the unit, 0-based (`SLOT=`).
    pub slot: i64,
    /// Filament at the gate, from that unit's own `gate_status`.
    pub occupied: bool,
    pub material: String,
    pub vendor: String,
    /// Label from the spool table, e.g. `Rapid PETG Transparent`.
    pub label: String,
    /// `#RRGGBB`, or empty when the slot declares no colour.
    pub color: String,
    /// Remaining weight, grams. 0 when unknown.
    pub weight: f64,
    /// Spoolman id, when the spool is synced.
    pub spool_id: Option<i64>,
    /// Key into `ace.spools`, kept so a `spools`-only delta can refresh the slot.
    /// Bookkeeping, not display: the card never sees it.
    #[serde(skip)]
    pub spool_key: String,
}

/// One toolhead and the ACE slot feeding it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AceHead {
    /// Toolhead index, 0-based (`HEAD=`, and `T0`…`T3` on the printer).
    pub index: i64,
    /// Unit feeding this head.
    pub ace: i64,
    /// Slot it is fed from. `None` = nothing loaded (`head_source[h]` is null).
    pub slot: Option<i64>,
    pub material: String,
    pub subtype: String,
    pub vendor: String,
    /// `#RRGGBB` or empty.
    pub color: String,
    /// Manual / TPU bypass: no ACE feed, RFID or feed assist.
    pub manual: bool,
    /// Head mode: loads through the printer's own side feeder.
    pub feeder: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AceState {
    /// Engine contract version; the card degrades on anything it does not know.
    pub api_version: i64,
    /// Aggregate of the active device: `ready`, `busy` or `unknown`.
    pub status: String,
    /// `normal` (stock feeders), `multi` (slot N feeds head N on every unit) or
    /// `head` (each head wired to one unit, swapping between its four slots).
    pub mode: String,
    /// Units detected, 1..4.
    pub device_count: i64,
    /// Unit the bare commands act on; moved with `ACE_SWITCH TARGET=n`.
    pub active_device: i64,
    /// Ambient temperature of the active unit, °C.
    pub temp: f64,
    /// Dryer of the active unit.
    pub dryer: AceDryer,
    /// `idle`, `unload`, `load`, `flush`, `done`.
    pub swap_phase: String,
    pub swap_in_progress: bool,
    pub unload_all_active: bool,
    /// Failure of the most recent swap, already phrased; empty when it went fine.
    pub last_swap_result: String,
    /// Bowden calibration session: `idle`, `running`, `failed`…
    pub calibration_state: String,
    /// Spoolman auto-sync is on.
    pub spoolman: bool,
    pub units: Vec<AceUnit>,
    pub slots: Vec<AceSlot>,
    pub heads: Vec<AceHead>,
}

impl AceState {
    /// Present when MultiACE is installed on the printer.
    pub fn present(objects: &[String]) -> bool {
        objects.iter().any(|o| o == "ace")
    }

    pub fn subscription(objects: &mut Value) {
        objects["ace"] = json!(ACE_FIELDS);
    }

    fn apply(&mut self, status: &Value) {
        let Some(a) = status.get("ace") else { return };
        set_i64(&mut self.api_version, a.get("api_version"));
        set_str(&mut self.status, a.get("status"));
        set_str(&mut self.mode, a.get("mode"));
        set_i64(&mut self.device_count, a.get("device_count"));
        set_i64(&mut self.active_device, a.get("active_device"));
        set_f64(&mut self.temp, a.get("temp"));
        set_str(&mut self.swap_phase, a.get("swap_phase"));
        set_bool(&mut self.swap_in_progress, a.get("swap_in_progress"));
        set_bool(&mut self.unload_all_active, a.get("unload_all_active"));
        set_bool(&mut self.spoolman, a.get("spoolman_auto"));
        if let Some(v) = a.get("last_swap_result") {
            self.last_swap_result = swap_failure(v);
        }
        if let Some(d) = a.get("dryer_status") {
            apply_dryer(&mut self.dryer, d);
        }
        if let Some(c) = a.get("calibration") {
            set_str(&mut self.calibration_state, c.get("state"));
        }
        self.apply_units(a);
        self.apply_spools(a);
        self.apply_heads(a);
    }

    /// `aces[]` carries every unit's own gates and slots, so a second unit is
    /// reported as fully as the active one (the top-level `gate_status` is the
    /// active device alone).
    fn apply_units(&mut self, a: &Value) {
        let Some(aces) = a.get("aces").and_then(Value::as_array) else { return };
        self.units.clear();
        for u in aces {
            let idx = u.get("idx").and_then(Value::as_i64).unwrap_or(0);
            let mut unit = AceUnit { index: idx, ..Default::default() };
            set_bool(&mut unit.connected, u.get("connected"));
            set_f64(&mut unit.temp, u.get("temp"));
            unit.humidity = u.get("humidity").and_then(Value::as_f64);
            if let Some(d) = u.get("dryer_status") {
                apply_dryer(&mut unit.dryer, d);
            }
            self.units.push(unit);

            let gates: Vec<bool> = u
                .get("gate_status")
                .and_then(Value::as_array)
                .map(|g| g.iter().map(|v| v.as_i64().map(|n| n != 0).or_else(|| v.as_bool()).unwrap_or(false)).collect())
                .unwrap_or_default();

            if let Some(slots) = u.get("slots").and_then(Value::as_array) {
                for (pos, s) in slots.iter().enumerate() {
                    let i = s.get("index").and_then(Value::as_i64).unwrap_or(pos as i64);
                    let material = s.get("material").and_then(Value::as_str).unwrap_or("").to_string();
                    let color = rgb_color(s.get("color"), !material.is_empty());
                    let slot = self.slot_mut(idx, i);
                    slot.occupied = gates.get(pos).copied().unwrap_or(false);
                    slot.material = material;
                    set_str(&mut slot.vendor, s.get("brand"));
                    if let Some(c) = color {
                        slot.color = c;
                    }
                }
            } else {
                for (pos, occupied) in gates.iter().enumerate() {
                    self.slot_mut(idx, pos as i64).occupied = *occupied;
                }
            }
        }
        self.units.sort_by_key(|u| u.index);
    }

    /// `spool_binding` maps `"<ace>_<slot>"` to a key in the `spools` table. The
    /// table is the curated side (label, weight, Spoolman id) and wins over what
    /// the unit reports for a bound slot.
    fn apply_spools(&mut self, a: &Value) {
        if let Some(binding) = a.get("spool_binding").and_then(Value::as_object) {
            for (key, v) in binding {
                let Some((ace, slot)) = parse_slot_key(key) else { continue };
                self.slot_mut(ace, slot).spool_key = v.as_str().unwrap_or("").to_string();
            }
        }
        let Some(spools) = a.get("spools").and_then(Value::as_object) else { return };
        for s in &mut self.slots {
            let Some(sp) = spools.get(&s.spool_key) else { continue };
            set_str(&mut s.material, sp.get("material"));
            set_str(&mut s.vendor, sp.get("vendor"));
            set_str(&mut s.label, sp.get("label"));
            set_f64(&mut s.weight, sp.get("weight_g"));
            if let Some(c) = hex_color(sp.get("color")) {
                s.color = c;
            }
            if let Some(id) = sp.get("spoolman_id") {
                s.spool_id = id.as_i64().or_else(|| id.as_str().and_then(|t| t.parse().ok()));
            }
        }
        self.slots.sort_by_key(|s| (s.ace, s.slot));
    }

    fn apply_heads(&mut self, a: &Value) {
        // The three per-head maps are each keyed by every head.
        for field in ["head_manual", "head_feeder", "head_source"] {
            let Some(m) = a.get(field).and_then(Value::as_object) else { continue };
            for key in m.keys() {
                if let Ok(i) = key.parse::<i64>() {
                    self.head_mut(i);
                }
            }
        }
        // `head_source` arrives whole and a head's entry is null when unloaded.
        if let Some(src) = a.get("head_source").and_then(Value::as_object) {
            for h in &mut self.heads {
                h.slot = None;
                h.material.clear();
                h.subtype.clear();
                h.vendor.clear();
                h.color.clear();
            }
            for (key, v) in src {
                let Ok(i) = key.parse::<i64>() else { continue };
                if v.is_null() {
                    continue;
                }
                let h = self.head_mut(i);
                h.slot = v.get("slot").and_then(Value::as_i64);
                set_i64(&mut h.ace, v.get("ace_index"));
                set_str(&mut h.material, v.get("type"));
                set_str(&mut h.subtype, v.get("subtype"));
                set_str(&mut h.vendor, v.get("brand"));
                if let Some(c) = hex_color(v.get("color")) {
                    h.color = c;
                }
            }
        }
        for (field, manual) in [("head_manual", true), ("head_feeder", false)] {
            let Some(m) = a.get(field).and_then(Value::as_object) else { continue };
            for (key, v) in m {
                let Ok(i) = key.parse::<i64>() else { continue };
                let on = v.as_bool().unwrap_or(false);
                let h = self.head_mut(i);
                if manual {
                    h.manual = on;
                } else {
                    h.feeder = on;
                }
            }
        }
        // `head_ace` is the head-mode wiring. In multi mode slot N feeds head N on
        // whichever unit is active, and the field still holds its default 0..3
        // mapping, so reading it there would point heads at units that may not exist.
        if self.mode == "head" {
            if let Some(m) = a.get("head_ace").and_then(Value::as_object) {
                for (key, v) in m {
                    let Ok(i) = key.parse::<i64>() else { continue };
                    let Some(n) = v.as_i64() else { continue };
                    self.head_mut(i).ace = n;
                }
            }
        }
        self.heads.sort_by_key(|h| h.index);
    }

    fn slot_mut(&mut self, ace: i64, slot: i64) -> &mut AceSlot {
        match self.slots.iter().position(|s| s.ace == ace && s.slot == slot) {
            Some(i) => &mut self.slots[i],
            None => {
                self.slots.push(AceSlot { ace, slot, ..Default::default() });
                self.slots.last_mut().unwrap()
            }
        }
    }

    fn head_mut(&mut self, index: i64) -> &mut AceHead {
        match self.heads.iter().position(|h| h.index == index) {
            Some(i) => &mut self.heads[i],
            None => {
                self.heads.push(AceHead { index, ..Default::default() });
                self.heads.last_mut().unwrap()
            }
        }
    }
}

fn apply_dryer(dryer: &mut AceDryer, d: &Value) {
    set_str(&mut dryer.status, d.get("status"));
    set_f64(&mut dryer.target_temp, d.get("target_temp"));
    set_i64(&mut dryer.duration, d.get("duration"));
    set_i64(&mut dryer.remain_time, d.get("remain_time"));
}

/// `"0_2"` → unit 0, slot 2.
fn parse_slot_key(key: &str) -> Option<(i64, i64)> {
    let (ace, slot) = key.split_once('_')?;
    Some((ace.parse().ok()?, slot.parse().ok()?))
}

/// `last_swap_result` is `{head, ace, slot, status, ts}`. `ok` is not worth
/// showing, so only a failure comes back phrased.
fn swap_failure(v: &Value) -> String {
    let Some(o) = v.as_object() else {
        return v.as_str().unwrap_or("").to_string();
    };
    let status = o.get("status").and_then(Value::as_str).unwrap_or("");
    if status.is_empty() || status == "ok" {
        return String::new();
    }
    match (
        o.get("head").and_then(Value::as_i64),
        o.get("ace").and_then(Value::as_i64),
        o.get("slot").and_then(Value::as_i64),
    ) {
        (Some(h), Some(a), Some(s)) => format!("{status} on T{h} (ACE {a} slot {s})"),
        (Some(h), _, _) => format!("{status} on T{h}"),
        _ => status.to_string(),
    }
}

/// `"E8F1F5"` or `"#E8F1F5"` → `"#E8F1F5"`. `None` when the field is absent.
fn hex_color(v: Option<&Value>) -> Option<String> {
    let c = v?.as_str()?.trim();
    if c.is_empty() {
        return Some(String::new());
    }
    Some(if c.starts_with('#') { c.to_string() } else { format!("#{c}") })
}

/// The unit's raw `[r, g, b]` triple. `[0, 0, 0]` is the tag's black only when the
/// slot declares a material; on its own it means the slot declares no colour.
fn rgb_color(v: Option<&Value>, has_material: bool) -> Option<String> {
    let a = v?.as_array()?;
    let c: Vec<i64> = a.iter().filter_map(Value::as_i64).collect();
    if c.len() < 3 {
        return None;
    }
    if c[..3] == [0, 0, 0] && !has_material {
        return Some(String::new());
    }
    Some(format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]))
}

fn set_opt_str(dst: &mut Option<String>, v: Option<&Value>) {
    if let Some(v) = v {
        *dst = v.as_str().filter(|s| !s.is_empty()).map(str::to_string);
    }
}

fn set_bool(dst: &mut bool, v: Option<&Value>) {
    if let Some(b) = v.and_then(Value::as_bool) {
        *dst = b;
    }
}

fn set_i64(dst: &mut i64, v: Option<&Value>) {
    if let Some(n) = v.and_then(Value::as_i64) {
        *dst = n;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Heater {
    pub temperature: f64,
    pub target: f64,
    /// 0..1 duty cycle when known.
    pub power: f64,
}

/// Everything the UI needs about one printer, refreshed on every Moonraker update.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Snapshot {
    pub connection: ConnectionState,
    /// `ready`, `startup`, `shutdown`, `error` or `disconnected` (Klippy host state).
    pub klippy_state: String,
    pub klippy_message: String,
    /// `standby`, `printing`, `paused`, `complete`, `cancelled`, `error`.
    pub print_state: String,
    pub filename: String,
    /// 0..1
    pub progress: f64,
    /// Seconds actually spent printing (excludes pauses and heating).
    pub print_duration: f64,
    /// Seconds since the job started.
    pub total_duration: f64,
    /// Slicer estimate from file metadata, seconds.
    pub estimated_time: Option<f64>,
    /// Best-effort remaining time, seconds.
    pub time_left: Option<f64>,
    /// Filament extruded so far, mm.
    pub filament_used: f64,
    pub current_layer: Option<u32>,
    pub total_layer: Option<u32>,
    pub extruder: Heater,
    pub bed: Heater,
    pub chamber: Option<Heater>,
    /// Part cooling fan, 0..1.
    pub fan_speed: f64,
    /// 1.0 == 100 %.
    pub speed_factor: f64,
    pub flow_factor: f64,
    /// Last `M117` message.
    pub message: String,
    pub hostname: String,
    pub software_version: String,
    /// Public `gcode_macro` sections (names not starting with `_`), sorted.
    pub macros: Vec<MacroInfo>,
    /// Preheat presets from Mainsail / Fluidd.
    pub presets: Vec<Preset>,
    /// Script that turns everything off (Mainsail setting, default `TURN_OFF_HEATERS`).
    pub cooldown_gcode: String,
    /// Present when the AFC add-on is installed on the printer.
    pub afc: Option<AfcState>,
    /// Present when MultiACE is installed on the printer.
    pub ace: Option<AceState>,
    /// Every heater / sensor / temperature fan on the printer, in discovery order.
    pub temperatures: Vec<TempReading>,
    /// Webcams configured in Moonraker.
    pub webcams: Vec<Webcam>,
    /// Slicer thumbnail of the current file, as a path under the `gcodes` root.
    pub thumbnail: Option<String>,
    /// First filament color / type written by the slicer in the file metadata.
    pub filament_color: Option<String>,
    pub filament_type: Option<String>,
    /// Every filament color in the file, one per tool (multi-tool / toolchanger prints).
    pub filament_colors: Vec<String>,
    /// Extruder the toolhead is using: `extruder`, `extruder1`… (toolchangers).
    pub active_extruder: String,
    pub machine: Machine,
    /// Unix epoch, milliseconds.
    pub last_update: u64,
    pub last_error: Option<String>,
}

impl Snapshot {
    pub fn is_busy(&self) -> bool {
        matches!(self.print_state.as_str(), "printing" | "paused")
    }

    pub fn touch(&mut self) {
        self.last_update = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
    }

    /// Recompute `time_left` from the slicer estimate when available, otherwise
    /// extrapolate from the progress so far.
    pub fn recompute_time_left(&mut self) {
        if !self.is_busy() {
            self.time_left = None;
            return;
        }
        self.time_left = match self.estimated_time {
            Some(est) if est > 0.0 => Some((est - self.print_duration).max(0.0)),
            _ if self.progress > 0.02 => {
                Some((self.print_duration / self.progress - self.print_duration).max(0.0))
            }
            _ => None,
        };
    }

    /// Merge a Moonraker `status` object (from a subscribe response or a
    /// `notify_status_update`) into this snapshot. Only keys present in `status`
    /// are touched, which is how Moonraker sends deltas.
    pub fn apply_status(
        &mut self,
        status: &Value,
        chamber_key: Option<&str>,
        afc_keys: Option<&AfcKeys>,
        ace: bool,
    ) {
        if let Some(keys) = afc_keys {
            self.afc.get_or_insert_with(AfcState::default).apply(status, keys);
        }
        if ace {
            self.ace.get_or_insert_with(AceState::default).apply(status);
        }
        for reading in &mut self.temperatures {
            if let Some(v) = status.get(&reading.object) {
                set_f64(&mut reading.temperature, v.get("temperature"));
                if let Some(t) = v.get("target").and_then(Value::as_f64) {
                    reading.target = Some(t);
                }
                if let Some(p) = v.get("power").and_then(Value::as_f64) {
                    reading.power = Some(p);
                }
                if let Some(sp) = v.get("speed").and_then(Value::as_f64) {
                    reading.speed = Some(sp);
                }
            }
        }
        if let Some(w) = status.get("webhooks") {
            set_str(&mut self.klippy_state, w.get("state"));
            set_str(&mut self.klippy_message, w.get("state_message"));
        }
        if let Some(p) = status.get("print_stats") {
            set_str(&mut self.print_state, p.get("state"));
            set_str(&mut self.filename, p.get("filename"));
            set_f64(&mut self.print_duration, p.get("print_duration"));
            set_f64(&mut self.total_duration, p.get("total_duration"));
            set_f64(&mut self.filament_used, p.get("filament_used"));
            if let Some(info) = p.get("info") {
                if let Some(v) = info.get("current_layer") {
                    self.current_layer = v.as_u64().map(|n| n as u32);
                }
                if let Some(v) = info.get("total_layer") {
                    if let Some(n) = v.as_u64() {
                        self.total_layer = Some(n as u32);
                    }
                }
            }
        }
        if let Some(d) = status.get("display_status") {
            set_f64(&mut self.progress, d.get("progress"));
            set_str(&mut self.message, d.get("message"));
        } else if let Some(v) = status.get("virtual_sdcard") {
            // Fallback when display_status is not reported.
            set_f64(&mut self.progress, v.get("progress"));
        }
        if let Some(e) = status.get("extruder") {
            apply_heater(&mut self.extruder, e);
        }
        if let Some(b) = status.get("heater_bed") {
            apply_heater(&mut self.bed, b);
        }
        if let Some(key) = chamber_key {
            if let Some(c) = status.get(key) {
                let h = self.chamber.get_or_insert_with(Heater::default);
                apply_heater(h, c);
            }
        }
        if let Some(f) = status.get("fan") {
            set_f64(&mut self.fan_speed, f.get("speed"));
        }
        if let Some(e) = status.get("toolhead").and_then(|t| t.get("extruder")).and_then(Value::as_str) {
            self.active_extruder = e.to_string();
        }
        if let Some(g) = status.get("gcode_move") {
            set_f64(&mut self.speed_factor, g.get("speed_factor"));
            set_f64(&mut self.flow_factor, g.get("extrude_factor"));
        }
        self.progress = self.progress.clamp(0.0, 1.0);
        self.recompute_time_left();
        self.touch();
    }
}

fn apply_heater(h: &mut Heater, v: &Value) {
    set_f64(&mut h.temperature, v.get("temperature"));
    set_f64(&mut h.target, v.get("target"));
    set_f64(&mut h.power, v.get("power"));
}

fn set_str(dst: &mut String, v: Option<&Value>) {
    if let Some(s) = v.and_then(Value::as_str) {
        *dst = s.to_string();
    }
}

fn set_f64(dst: &mut f64, v: Option<&Value>) {
    if let Some(n) = v.and_then(Value::as_f64) {
        *dst = n;
    }
}

/// Payload of the `printer:update` event.
#[derive(Debug, Clone, Serialize)]
pub struct PrinterUpdate {
    pub id: String,
    pub snapshot: Snapshot,
}
