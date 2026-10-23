//! Data model shared between the Rust backend and the web front.
//!
//! Everything here is serialised with `serde` and mirrored in `src/lib/types.ts`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub fn apply_status(&mut self, status: &Value, chamber_key: Option<&str>) {
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
