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
}

impl PrinterConfig {
    /// Websocket endpoint derived from the HTTP base URL.
    pub fn websocket_url(&self) -> Result<url::Url, String> {
        let mut u = url::Url::parse(self.url.trim()).map_err(|e| format!("URL invalide: {e}"))?;
        let scheme = match u.scheme() {
            "http" => "ws",
            "https" => "wss",
            "ws" | "wss" => u.scheme(),
            other => return Err(format!("Schéma non supporté: {other}")),
        }
        .to_string();
        u.set_scheme(&scheme).map_err(|_| "Schéma non supporté".to_string())?;
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
    /// Every heater / sensor / temperature fan on the printer, in discovery order.
    pub temperatures: Vec<TempReading>,
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
    pub fn apply_status(&mut self, status: &Value, chamber_key: Option<&str>, afc_keys: Option<&AfcKeys>) {
        if let Some(keys) = afc_keys {
            self.afc.get_or_insert_with(AfcState::default).apply(status, keys);
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
