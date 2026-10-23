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

use super::model::{ConnectionState, MacroInfo, PrinterConfig, PrinterUpdate, Snapshot};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = SplitSink<Ws, Message>;
type WsStream = SplitStream<Ws>;
type RpcResult = Result<Value, String>;
type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<RpcResult>>>>;

const RPC_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const KEEPALIVE: Duration = Duration::from_secs(20);
const MAX_BACKOFF_SECS: u64 = 15;

/// Printer objects we subscribe to. `None` means "every field".
fn subscription_objects(chamber_key: Option<&str>) -> Value {
    let mut objects = json!({
        "webhooks": null,
        "print_stats": null,
        "display_status": null,
        "virtual_sdcard": ["progress", "file_path"],
        "extruder": ["temperature", "target", "power"],
        "heater_bed": ["temperature", "target", "power"],
        "fan": ["speed"],
        "gcode_move": ["speed_factor", "extrude_factor"],
    });
    if let Some(key) = chamber_key {
        objects[key] = Value::Null;
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
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let msg = json!({ "jsonrpc": "2.0", "method": method, "params": params, "id": id });
        {
            let mut sink = self.sink.lock().await;
            sink.send(Message::Text(msg.to_string().into()))
                .await
                .map_err(|e| format!("Envoi impossible: {e}"))?;
        }

        match tokio::time::timeout(RPC_TIMEOUT, rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => Err("Connexion fermée".to_string()),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(format!("Délai dépassé ({method})"))
            }
        }
    }
}

/// Reads the socket, resolves pending RPC calls and forwards notifications.
async fn read_loop(mut stream: WsStream, pending: Pending, notif: mpsc::Sender<(String, Value)>) -> String {
    while let Some(item) = stream.next().await {
        let text = match item {
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Close(_)) => return "Fermé par le serveur".to_string(),
            Ok(_) => continue,
            Err(e) => return format!("Erreur websocket: {e}"),
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
                        .unwrap_or("Erreur Moonraker");
                    Err(message.to_string())
                } else {
                    Ok(msg.get("result").cloned().unwrap_or(Value::Null))
                };
                let _ = tx.send(res);
            }
        } else if let Some(method) = msg.get("method").and_then(Value::as_str) {
            let params = msg.get("params").cloned().unwrap_or(Value::Null);
            if notif.send((method.to_string(), params)).await.is_err() {
                return "Session terminée".to_string();
            }
        }
    }
    "Connexion perdue".to_string()
}

async fn connect(cfg: &PrinterConfig) -> Result<Ws, String> {
    let ws_url = cfg.websocket_url()?;
    let mut req = ws_url
        .as_str()
        .into_client_request()
        .map_err(|e| format!("Requête invalide: {e}"))?;
    if let Some(key) = cfg.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        let value = HeaderValue::from_str(key).map_err(|_| "Clé API invalide".to_string())?;
        req.headers_mut().insert("X-Api-Key", value);
    }
    let (ws, _) = tokio::time::timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(req))
        .await
        .map_err(|_| "Délai de connexion dépassé".to_string())?
        .map_err(|e| format!("Connexion impossible: {e}"))?;
    Ok(ws)
}

struct Session {
    cfg: PrinterConfig,
    rpc: Rpc,
    snapshot: Arc<RwLock<Snapshot>>,
    updates: mpsc::Sender<PrinterUpdate>,
    chamber_key: Option<String>,
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

    /// Discover optional objects (chamber sensor), subscribe and apply the initial status.
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

        self.discover_macros(&objects).await;

        let res = self
            .rpc
            .call(
                "printer.objects.subscribe",
                json!({ "objects": subscription_objects(self.chamber_key.as_deref()) }),
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
    }

    async fn apply_status(&mut self, status: &Value) {
        let filename = {
            let mut s = self.snapshot.write().await;
            s.apply_status(status, self.chamber_key.as_deref());
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

    async fn execute(&self, action: Action) -> RpcResult {
        match action {
            Action::Pause => self.rpc.call("printer.print.pause", json!({})).await,
            Action::Resume => self.rpc.call("printer.print.resume", json!({})).await,
            Action::Cancel => self.rpc.call("printer.print.cancel", json!({})).await,
            Action::EmergencyStop => self.rpc.call("printer.emergency_stop", json!({})).await,
            Action::FirmwareRestart => self.rpc.call("printer.firmware_restart", json!({})).await,
            Action::Gcode(script) => {
                self.rpc
                    .call("printer.gcode.script", json!({ "script": script }))
                    .await
            }
        }
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
                    Some(c) => { let _ = c.reply.send(Err("Imprimante hors ligne".into())); }
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
                    return Err(res.unwrap_or_else(|_| "Lecteur interrompu".into()));
                }
                notif = notif_rx.recv() => match notif {
                    Some((method, params)) => sess.handle_notification(&method, &params).await,
                    None => return Err("Connexion perdue".into()),
                },
                cmd = cmd_rx.recv() => match cmd {
                    Some(c) => {
                        let res = sess.execute(c.action).await;
                        let _ = c.reply.send(res);
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
            .unwrap_or_else(|| "inconnu".to_string());
        Ok::<String, String>(format!("{hostname} (Klippy: {klippy})"))
    }
    .await;

    reader.abort();
    result
}
