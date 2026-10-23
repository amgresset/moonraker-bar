//! Moonraker websocket client.
//!
//! One `run` task per configured printer: it connects to Moonraker's JSON-RPC
//! websocket, subscribes to the printer objects the UI needs, merges every
//! status delta into a shared [`Snapshot`], publishes an update on the
//! `updates` channel, executes commands received on `cmd_rx`, and reconnects
//! with a small backoff whenever the connection drops.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, watch, Mutex, RwLock};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::model::{
    AceState, AfcKeys, ConnectionState, MacroInfo, Machine, Preset, PresetTarget, PrinterConfig, PrinterUpdate, Snapshot,
    TempReading,
    Webcam,
};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = SplitSink<Ws, Message>;
type WsStream = SplitStream<Ws>;
type RpcResult = Result<Value, String>;
type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<RpcResult>>>>;

const RPC_TIMEOUT: Duration = Duration::from_secs(10);
/// G-code scripts only answer once Klipper has finished executing them, so a
/// homing, a QGL or a heat-soak macro can legitimately take minutes.
const GCODE_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const KEEPALIVE: Duration = Duration::from_secs(20);
const MAX_BACKOFF_SECS: u64 = 15;

/// Printer objects we subscribe to. `None` means "every field".
fn subscription_objects(chamber_key: Option<&str>, afc: Option<&AfcKeys>, ace: bool, temps: &[TempReading]) -> Value {
    let mut objects = json!({
        "webhooks": null,
        "print_stats": null,
        "display_status": null,
        "virtual_sdcard": ["progress", "file_path"],
        "extruder": ["temperature", "target", "power"],
        "heater_bed": ["temperature", "target", "power"],
        "fan": ["speed"],
        "gcode_move": ["speed_factor", "extrude_factor"],
        "toolhead": ["extruder"],
    });
    if let Some(key) = chamber_key {
        objects[key] = Value::Null;
    }
    if let Some(afc) = afc {
        afc.subscription(&mut objects);
    }
    if ace {
        AceState::subscription(&mut objects);
    }
    for t in temps {
        objects[&t.object] = TempReading::fields(&t.kind);
    }
    objects
}

/// Something the UI asked the printer to do.
#[derive(Debug)]
pub enum Action {
    Pause,
    Resume,
    Cancel,
    EmergencyStop,
    FirmwareRestart,
    Gcode(String),
    /// Apply a preheat preset by id (see `Snapshot::presets`).
    ApplyPreset(String),
    /// Run the cooldown script.
    Cooldown,
    /// Re-read presets from the Moonraker database.
    ReloadPresets,
}

#[derive(Debug)]
pub struct Command {
    pub action: Action,
    pub reply: oneshot::Sender<RpcResult>,
}

/// Thin JSON-RPC layer over the write half of the socket.
#[derive(Clone)]
struct Rpc {
    sink: Arc<Mutex<WsSink>>,
    pending: Pending,
    next_id: Arc<AtomicU64>,
}

impl Rpc {
    async fn call(&self, method: &str, params: Value) -> RpcResult {
        self.call_with_timeout(method, params, RPC_TIMEOUT).await
    }

    async fn call_with_timeout(&self, method: &str, params: Value, timeout: Duration) -> RpcResult {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let msg = json!({ "jsonrpc": "2.0", "method": method, "params": params, "id": id });
        {
            let mut sink = self.sink.lock().await;
            sink.send(Message::Text(msg.to_string().into()))
                .await
                .map_err(|e| format!("Send failed: {e}"))?;
        }

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => Err("Connection closed".to_string()),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(format!("Timed out ({method})"))
            }
        }
    }
}

/// Reads the socket, resolves pending RPC calls and forwards notifications.
async fn read_loop(mut stream: WsStream, pending: Pending, notif: mpsc::Sender<(String, Value)>) -> String {
    while let Some(item) = stream.next().await {
        let text = match item {
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Close(_)) => return "Closed by server".to_string(),
            Ok(_) => continue,
            Err(e) => return format!("Websocket error: {e}"),
        };
        let msg: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(id) = msg.get("id").and_then(Value::as_u64) {
            if let Some(tx) = pending.lock().await.remove(&id) {
                let res = if let Some(err) = msg.get("error") {
                    let message = err
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("Moonraker error");
                    Err(message.to_string())
                } else {
                    Ok(msg.get("result").cloned().unwrap_or(Value::Null))
                };
                let _ = tx.send(res);
            }
        } else if let Some(method) = msg.get("method").and_then(Value::as_str) {
            let params = msg.get("params").cloned().unwrap_or(Value::Null);
            if notif.send((method.to_string(), params)).await.is_err() {
                return "Session ended".to_string();
            }
        }
    }
    "Connection lost".to_string()
}

async fn connect(cfg: &PrinterConfig) -> Result<Ws, String> {
    let ws_url = cfg.websocket_url()?;
    let mut req = ws_url
        .as_str()
        .into_client_request()
        .map_err(|e| format!("Invalid request: {e}"))?;
    if let Some(key) = cfg.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        let value = HeaderValue::from_str(key).map_err(|_| "Invalid API key".to_string())?;
        req.headers_mut().insert("X-Api-Key", value);
    }
    let (ws, _) = tokio::time::timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(req))
        .await
        .map_err(|_| "Connection timed out".to_string())?
        .map_err(|e| format!("Cannot connect: {e}"))?;
    Ok(ws)
}

struct Session {
    cfg: PrinterConfig,
    rpc: Rpc,
    snapshot: Arc<RwLock<Snapshot>>,
    updates: mpsc::Sender<PrinterUpdate>,
    chamber_key: Option<String>,
    afc_keys: Option<AfcKeys>,
    /// MultiACE installed on this printer.
    ace: bool,
    metadata_for: String,
}

impl Session {
    async fn publish(&self) {
        let snap = self.snapshot.read().await.clone();
        // Never block the socket on a slow consumer; the UI only needs the latest state.
        let _ = self.updates.try_send(PrinterUpdate { id: self.cfg.id.clone(), snapshot: snap });
    }

    async fn set_connected(&self, info: &Value) {
        let mut s = self.snapshot.write().await;
        s.connection = ConnectionState::Connected;
        s.last_error = None;
        if let Some(st) = info.get("klippy_state").and_then(Value::as_str) {
            s.klippy_state = st.to_string();
        }
        if let Some(v) = info.get("moonraker_version").and_then(Value::as_str) {
            s.software_version = v.to_string();
        }
        s.touch();
    }

    /// Discover optional objects (chamber sensor, AFC, MultiACE), subscribe and
    /// apply the initial status.
    async fn subscribe(&mut self) -> Result<(), String> {
        if let Ok(info) = self.rpc.call("printer.info", json!({})).await {
            let mut s = self.snapshot.write().await;
            if let Some(h) = info.get("hostname").and_then(Value::as_str) {
                s.hostname = h.to_string();
            }
            if let Some(st) = info.get("state").and_then(Value::as_str) {
                s.klippy_state = st.to_string();
            }
            if let Some(m) = info.get("state_message").and_then(Value::as_str) {
                s.klippy_message = m.to_string();
            }
        }

        let list = self.rpc.call("printer.objects.list", json!({})).await?;
        let objects: Vec<String> = list
            .get("objects")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();

        self.chamber_key = objects
            .iter()
            .find(|o| {
                let lower = o.to_lowercase();
                (lower.starts_with("heater_generic ") || lower.starts_with("temperature_sensor "))
                    && lower.contains("chamber")
            })
            .cloned();

        self.afc_keys = AfcKeys::from_objects(&objects);
        if self.afc_keys.is_none() {
            self.snapshot.write().await.afc = None;
        }

        self.ace = AceState::present(&objects);
        if !self.ace {
            self.snapshot.write().await.ace = None;
        }

        // Every temperature-bearing object, so the user can pick which tiles to show.
        let temps: Vec<TempReading> = objects
            .iter()
            .filter_map(|o| {
                TempReading::is_temperature_object(o).map(|kind| TempReading {
                    object: o.clone(),
                    kind: kind.to_string(),
                    ..Default::default()
                })
            })
            .collect();
        self.snapshot.write().await.temperatures = temps;

        self.discover_macros(&objects).await;
        load_presets(&self.rpc, &self.snapshot).await;
        load_webcams(&self.rpc, &self.snapshot).await;

        let temps = self.snapshot.read().await.temperatures.clone();
        let res = self
            .rpc
            .call(
                "printer.objects.subscribe",
                json!({
                    "objects": subscription_objects(
                        self.chamber_key.as_deref(),
                        self.afc_keys.as_ref(),
                        self.ace,
                        &temps,
                    )
                }),
            )
            .await?;
        if let Some(status) = res.get("status") {
            self.apply_status(status).await;
        }
        Ok(())
    }

    /// Collect public `gcode_macro` objects and their `description` from the config.
    async fn discover_macros(&self, objects: &[String]) {
        let mut names: Vec<String> = objects
            .iter()
            .filter_map(|o| o.strip_prefix("gcode_macro "))
            .filter(|n| !n.starts_with('_'))
            .map(str::to_string)
            .collect();
        names.sort_unstable_by_key(|n| n.to_uppercase());

        // Descriptions live in configfile.settings["gcode_macro name"]["description"].
        let settings = self
            .rpc
            .call("printer.objects.query", json!({ "objects": { "configfile": ["settings"] } }))
            .await
            .ok()
            .and_then(|v| v.pointer("/status/configfile/settings").cloned())
            .unwrap_or(Value::Null);

        // Machine shape, for the illustration.
        let num = |section: &str, key: &str| {
            settings.get(section).and_then(|s| s.get(key)).and_then(Value::as_f64).unwrap_or(0.0)
        };
        let machine = Machine {
            kinematics: settings
                .pointer("/printer/kinematics")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            x_max: num("stepper_x", "position_max"),
            y_max: num("stepper_y", "position_max"),
            z_max: num("stepper_z", "position_max"),
            leveling: if objects.iter().any(|o| o == "quad_gantry_level") {
                "qgl".into()
            } else if objects.iter().any(|o| o == "z_tilt") {
                "z_tilt".into()
            } else {
                String::new()
            },
        };

        let macros: Vec<MacroInfo> = names
            .into_iter()
            .map(|name| {
                let description = settings
                    .get(format!("gcode_macro {}", name.to_lowercase()))
                    .and_then(|m| m.get("description"))
                    .and_then(Value::as_str)
                    .filter(|d| *d != "G-Code macro")
                    .unwrap_or("")
                    .to_string();
                MacroInfo { name, description }
            })
            .collect();

        let mut s = self.snapshot.write().await;
        s.macros = macros;
        s.machine = machine;
    }

    async fn apply_status(&mut self, status: &Value) {
        let filename = {
            let mut s = self.snapshot.write().await;
            s.apply_status(status, self.chamber_key.as_deref(), self.afc_keys.as_ref(), self.ace);
            s.filename.clone()
        };
        if filename != self.metadata_for {
            self.metadata_for = filename.clone();
            self.refresh_metadata(&filename).await;
        }
        self.publish().await;
    }

    async fn refresh_metadata(&self, filename: &str) {
        let mut s = self.snapshot.write().await;
        if filename.is_empty() {
            s.estimated_time = None;
            s.current_layer = None;
            s.total_layer = None;
            s.thumbnail = None;
            s.filament_color = None;
            s.filament_colors.clear();
            s.filament_type = None;
            s.recompute_time_left();
            return;
        }
        drop(s);
        let meta = self
            .rpc
            .call("server.files.metadata", json!({ "filename": filename }))
            .await
            .unwrap_or(Value::Null);
        let mut s = self.snapshot.write().await;
        s.estimated_time = meta.get("estimated_time").and_then(Value::as_f64).filter(|t| *t > 0.0);
        if s.total_layer.is_none() {
            s.total_layer = meta.get("layer_count").and_then(Value::as_u64).map(|n| n as u32);
        }
        s.thumbnail = pick_thumbnail(filename, &meta);
        s.filament_color = meta
            .get("filament_colors")
            .and_then(Value::as_array)
            .and_then(|a| a.iter().filter_map(Value::as_str).find(|c| !c.trim().is_empty()))
            .map(|c| c.trim().to_string());
        s.filament_colors = meta
            .get("filament_colors")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(|c| c.trim().to_string()).collect())
            .unwrap_or_default();
        s.filament_type = meta
            .get("filament_type")
            .and_then(Value::as_str)
            .map(|t| t.split(';').next().unwrap_or(t).trim().to_string())
            .filter(|t| !t.is_empty());
        s.recompute_time_left();
    }

    async fn handle_notification(&mut self, method: &str, params: &Value) {
        match method {
            "notify_status_update" => {
                if let Some(status) = params.get(0) {
                    self.apply_status(status).await;
                }
            }
            "notify_klippy_ready" => {
                if let Err(e) = self.subscribe().await {
                    log::warn!("[{}] resubscribe failed: {e}", self.cfg.name);
                }
                self.publish().await;
            }
            "notify_webcams_changed" => {
                load_webcams(&self.rpc, &self.snapshot).await;
                self.publish().await;
            }
            "notify_klippy_shutdown" | "notify_klippy_disconnected" => {
                {
                    let mut s = self.snapshot.write().await;
                    s.klippy_state = if method == "notify_klippy_shutdown" {
                        "shutdown".into()
                    } else {
                        "disconnected".into()
                    };
                    s.touch();
                }
                self.publish().await;
            }
            _ => {}
        }
    }

}

/// Run one printer action. Called from its own task so a long-running G-code
/// never blocks the session loop (status updates, other commands, keepalive).
async fn execute(rpc: Rpc, snapshot: Arc<RwLock<Snapshot>>, action: Action) -> RpcResult {
    match action {
        Action::ApplyPreset(id) => {
            let script = {
                let s = snapshot.read().await;
                s.presets.iter().find(|p| p.id == id).map(Preset::to_gcode)
            };
            let Some(script) = script else { return Err("Preset not found".into()) };
            if script.is_empty() {
                return Err("Empty preset".into());
            }
            rpc.call_with_timeout("printer.gcode.script", json!({ "script": script }), GCODE_TIMEOUT).await
        }
        Action::Cooldown => {
            let script = snapshot.read().await.cooldown_gcode.clone();
            let script = if script.is_empty() { "TURN_OFF_HEATERS".to_string() } else { script };
            rpc.call_with_timeout("printer.gcode.script", json!({ "script": script }), GCODE_TIMEOUT).await
        }
        Action::ReloadPresets => {
            load_presets(&rpc, &snapshot).await;
            Ok(Value::Null)
        }
        Action::Pause => rpc.call("printer.print.pause", json!({})).await,
        Action::Resume => rpc.call("printer.print.resume", json!({})).await,
        Action::Cancel => rpc.call("printer.print.cancel", json!({})).await,
        Action::EmergencyStop => rpc.call("printer.emergency_stop", json!({})).await,
        Action::FirmwareRestart => rpc.call("printer.firmware_restart", json!({})).await,
        Action::Gcode(script) => {
            rpc.call_with_timeout("printer.gcode.script", json!({ "script": script }), GCODE_TIMEOUT)
                .await
        }
    }
}

/// Read preheat presets from Mainsail (`mainsail/presets`) and Fluidd
/// (`fluidd/uiSettings`). A missing namespace is not an error.
async fn load_presets(rpc: &Rpc, snapshot: &Arc<RwLock<Snapshot>>) {
    let mut presets: Vec<Preset> = Vec::new();
    let mut cooldown = String::new();

    if let Ok(v) = rpc
        .call("server.database.get_item", json!({ "namespace": "mainsail", "key": "presets" }))
        .await
    {
        let value = v.get("value").cloned().unwrap_or(Value::Null);
        if let Some(c) = value.get("cooldownGcode").and_then(Value::as_str) {
            cooldown = c.trim().to_string();
        }
        if let Some(map) = value.get("presets").and_then(Value::as_object) {
            for (id, p) in map {
                let name = p.get("name").and_then(Value::as_str).unwrap_or("").trim().to_string();
                if name.is_empty() {
                    continue;
                }
                let mut targets = Vec::new();
                if let Some(values) = p.get("values").and_then(Value::as_object) {
                    for (object, t) in values {
                        let enabled = t.get("bool").and_then(Value::as_bool).unwrap_or(false);
                        let value = t.get("value").and_then(Value::as_f64).unwrap_or(0.0);
                        if !enabled {
                            continue;
                        }
                        let kind = t.get("type").and_then(Value::as_str).unwrap_or("heater");
                        targets.push(PresetTarget { object: object.clone(), kind: kind.to_string(), value });
                    }
                }
                targets.sort_by_key(|t| heater_rank(&t.object));
                presets.push(Preset {
                    id: format!("mainsail:{id}"),
                    name,
                    source: "mainsail".into(),
                    targets,
                    gcode: p.get("gcode").and_then(Value::as_str).unwrap_or("").to_string(),
                });
            }
        }
    }

    if let Ok(v) = rpc
        .call("server.database.get_item", json!({ "namespace": "fluidd", "key": "uiSettings" }))
        .await
    {
        if let Some(list) = v.pointer("/value/dashboard/tempPresets").and_then(Value::as_array) {
            for (i, p) in list.iter().enumerate() {
                let name = p.get("name").and_then(Value::as_str).unwrap_or("").trim().to_string();
                if name.is_empty() {
                    continue;
                }
                let mut targets = Vec::new();
                if let Some(values) = p.get("values").and_then(Value::as_object) {
                    for (object, t) in values {
                        let active = t.get("active").and_then(Value::as_bool).unwrap_or(true);
                        let value = t.get("value").and_then(Value::as_f64).unwrap_or(0.0);
                        if !active {
                            continue;
                        }
                        let kind = match t.get("type").and_then(Value::as_str) {
                            Some("fan") => "temperature_fan",
                            _ => "heater",
                        };
                        targets.push(PresetTarget { object: object.clone(), kind: kind.to_string(), value });
                    }
                }
                targets.sort_by_key(|t| heater_rank(&t.object));
                let id = p.get("id").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| i.to_string());
                presets.push(Preset {
                    id: format!("fluidd:{id}"),
                    name,
                    source: "fluidd".into(),
                    targets,
                    gcode: p.get("gcode").and_then(Value::as_str).unwrap_or("").to_string(),
                });
            }
        }
    }

    presets.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    let mut s = snapshot.write().await;
    s.presets = presets;
    s.cooldown_gcode = if cooldown.is_empty() { "TURN_OFF_HEATERS".into() } else { cooldown };
}

/// The slicer thumbnail closest to 300 px wide, as a path under the `gcodes` root.
/// Thumbnail paths in the metadata are relative to the G-code file's folder.
fn pick_thumbnail(filename: &str, meta: &Value) -> Option<String> {
    let thumbs = meta.get("thumbnails")?.as_array()?;
    let best = thumbs
        .iter()
        .filter_map(|t| {
            let w = t.get("width")?.as_u64()?;
            let p = t.get("relative_path")?.as_str()?;
            Some((w, p))
        })
        .min_by_key(|(w, _)| (*w as i64 - 300).abs())?;
    let dir = filename.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    Some(if dir.is_empty() { best.1.to_string() } else { format!("{dir}/{}", best.1) })
}

/// Read the webcams Moonraker knows about (`server.webcams.list`). Missing on old
/// Moonraker versions: then the list stays empty and the manual camera URL is used.
async fn load_webcams(rpc: &Rpc, snapshot: &Arc<RwLock<Snapshot>>) {
    let Ok(v) = rpc.call("server.webcams.list", json!({})).await else { return };
    let webcams: Vec<Webcam> = v
        .get("webcams")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|w| {
                    let s = |k: &str| w.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                    let b = |k: &str| w.get(k).and_then(Value::as_bool).unwrap_or(false);
                    Webcam {
                        name: s("name"),
                        enabled: w.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                        service: s("service"),
                        stream_url: s("stream_url"),
                        snapshot_url: s("snapshot_url"),
                        flip_horizontal: b("flip_horizontal"),
                        flip_vertical: b("flip_vertical"),
                        rotation: w.get("rotation").and_then(Value::as_i64).unwrap_or(0),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    snapshot.write().await.webcams = webcams;
}

/// Display order: extruder, bed, then the rest.
fn heater_rank(object: &str) -> u8 {
    match object {
        "extruder" => 0,
        "heater_bed" => 1,
        o if o.starts_with("extruder") => 0,
        o if o.starts_with("heater_generic") => 2,
        _ => 3,
    }
}

/// Drive one printer until `stop` flips to `true`.
pub async fn run(
    cfg: PrinterConfig,
    snapshot: Arc<RwLock<Snapshot>>,
    updates: mpsc::Sender<PrinterUpdate>,
    mut cmd_rx: mpsc::Receiver<Command>,
    mut stop: watch::Receiver<bool>,
) {
    let mut backoff = 2u64;
    loop {
        if *stop.borrow() {
            return;
        }
        {
            let mut s = snapshot.write().await;
            s.connection = ConnectionState::Connecting;
            s.touch();
        }
        let _ = updates.try_send(PrinterUpdate { id: cfg.id.clone(), snapshot: snapshot.read().await.clone() });

        let outcome = match connect(&cfg).await {
            Ok(ws) => {
                backoff = 2;
                session(&cfg, ws, &snapshot, &updates, &mut cmd_rx, &mut stop).await
            }
            Err(e) => Err(e),
        };
        if *stop.borrow() {
            return;
        }

        {
            let mut s = snapshot.write().await;
            s.connection = ConnectionState::Offline;
            s.last_error = outcome.err();
            s.time_left = None;
            s.touch();
        }
        let _ = updates.try_send(PrinterUpdate { id: cfg.id.clone(), snapshot: snapshot.read().await.clone() });

        // Wait before reconnecting, but keep answering commands with an error meanwhile.
        let sleep = tokio::time::sleep(Duration::from_secs(backoff));
        tokio::pin!(sleep);
        loop {
            tokio::select! {
                _ = &mut sleep => break,
                _ = stop.changed() => return,
                cmd = cmd_rx.recv() => match cmd {
                    Some(c) => { let _ = c.reply.send(Err("Printer offline".into())); }
                    None => return,
                },
            }
        }
        backoff = (backoff * 2).min(MAX_BACKOFF_SECS);
    }
}

async fn session(
    cfg: &PrinterConfig,
    ws: Ws,
    snapshot: &Arc<RwLock<Snapshot>>,
    updates: &mpsc::Sender<PrinterUpdate>,
    cmd_rx: &mut mpsc::Receiver<Command>,
    stop: &mut watch::Receiver<bool>,
) -> Result<(), String> {
    let (sink, stream) = ws.split();
    let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
    let (notif_tx, mut notif_rx) = mpsc::channel::<(String, Value)>(128);
    let mut reader = tokio::spawn(read_loop(stream, pending.clone(), notif_tx));

    let rpc = Rpc { sink: Arc::new(Mutex::new(sink)), pending, next_id: Arc::new(AtomicU64::new(1)) };
    let mut sess = Session {
        cfg: cfg.clone(),
        rpc,
        snapshot: snapshot.clone(),
        updates: updates.clone(),
        chamber_key: None,
        afc_keys: None,
        ace: false,
        metadata_for: String::new(),
    };

    let result: Result<(), String> = async {
        let info = sess.rpc.call("server.info", json!({})).await?;
        sess.set_connected(&info).await;
        let klippy_ready = info.get("klippy_state").and_then(Value::as_str) == Some("ready");
        if klippy_ready {
            sess.subscribe().await?;
        }
        sess.publish().await;
        log::info!("[{}] connected", cfg.name);

        let mut keepalive = tokio::time::interval(KEEPALIVE);
        keepalive.tick().await; // first tick fires immediately; skip it

        loop {
            tokio::select! {
                _ = stop.changed() => return Ok(()),
                res = &mut reader => {
                    return Err(res.unwrap_or_else(|_| "Reader stopped".into()));
                }
                notif = notif_rx.recv() => match notif {
                    Some((method, params)) => sess.handle_notification(&method, &params).await,
                    None => return Err("Connection lost".into()),
                },
                cmd = cmd_rx.recv() => match cmd {
                    Some(c) => {
                        let rpc = sess.rpc.clone();
                        let snap = sess.snapshot.clone();
                        let updates = sess.updates.clone();
                        let id = sess.cfg.id.clone();
                        tokio::spawn(async move {
                            let res = execute(rpc, snap.clone(), c.action).await;
                            let _ = c.reply.send(res);
                            // Presets may have changed; push the fresh snapshot to the UI.
                            let _ = updates.try_send(PrinterUpdate { id, snapshot: snap.read().await.clone() });
                        });
                    }
                    None => return Ok(()),
                },
                _ = keepalive.tick() => {
                    // Also flushes pongs queued by the read half.
                    match sess.rpc.call("server.info", json!({})).await {
                        Ok(info) => {
                            let st = info.get("klippy_state").and_then(Value::as_str).unwrap_or("");
                            let mut s = sess.snapshot.write().await;
                            if !st.is_empty() && s.klippy_state != st {
                                s.klippy_state = st.to_string();
                                s.touch();
                            }
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
        }
    }
    .await;

    reader.abort();
    log::info!("[{}] disconnected: {:?}", cfg.name, result);
    result
}

/// One-shot connectivity test used by the settings form. Returns the printer hostname.
pub async fn probe(cfg: &PrinterConfig) -> Result<String, String> {
    let ws = connect(cfg).await?;
    let (sink, stream) = ws.split();
    let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
    let (notif_tx, _notif_rx) = mpsc::channel(8);
    let reader = tokio::spawn(read_loop(stream, pending.clone(), notif_tx));
    let rpc = Rpc { sink: Arc::new(Mutex::new(sink)), pending, next_id: Arc::new(AtomicU64::new(1)) };

    let result = async {
        let info = rpc.call("server.info", json!({})).await?;
        let klippy = info.get("klippy_state").and_then(Value::as_str).unwrap_or("?").to_string();
        let hostname = rpc
            .call("printer.info", json!({}))
            .await
            .ok()
            .and_then(|p| p.get("hostname").and_then(Value::as_str).map(str::to_string))
            .unwrap_or_else(|| "unknown".to_string());
        Ok::<String, String>(format!("{hostname} (Klippy: {klippy})"))
    }
    .await;

    reader.abort();
    result
}
