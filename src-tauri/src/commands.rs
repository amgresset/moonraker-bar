//! Tauri commands exposed to the web front (`invoke("...")`).

use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::manager::Manager;
use crate::moonraker::{self, Action, PrinterConfig, Snapshot};
use crate::tray;

type Mgr<'a> = State<'a, Arc<Manager>>;

/// Printer fields as typed in the settings form.
#[derive(Debug, Deserialize)]
pub struct PrinterInput {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub camera_url: Option<String>,
    #[serde(default)]
    pub web_url: Option<String>,
}

impl PrinterInput {
    fn into_config(self, id: String, pinned_macros: Vec<String>) -> Result<PrinterConfig, String> {
        let name = self.name.trim().to_string();
        let url = self.url.trim().trim_end_matches('/').to_string();
        if url.is_empty() {
            return Err("L'URL Moonraker est requise".into());
        }
        let cfg = PrinterConfig {
            id,
            name: if name.is_empty() { url.clone() } else { name },
            url,
            api_key: clean(self.api_key),
            camera_url: clean(self.camera_url),
            web_url: clean(self.web_url),
            pinned_macros,
        };
        cfg.websocket_url()?; // validate
        Ok(cfg)
    }
}

fn clean(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[tauri::command]
pub async fn list_printers(mgr: Mgr<'_>) -> Result<Vec<PrinterConfig>, String> {
    Ok(mgr.list().await)
}

#[tauri::command]
pub async fn get_snapshots(mgr: Mgr<'_>) -> Result<HashMap<String, Snapshot>, String> {
    Ok(mgr.snapshots().await)
}

#[tauri::command]
pub async fn add_printer(mgr: Mgr<'_>, input: PrinterInput) -> Result<PrinterConfig, String> {
    let cfg = input.into_config(String::new(), Vec::new())?;
    Ok(mgr.add(cfg).await)
}

#[tauri::command]
pub async fn update_printer(mgr: Mgr<'_>, id: String, input: PrinterInput) -> Result<PrinterConfig, String> {
    let pinned = mgr
        .list()
        .await
        .into_iter()
        .find(|p| p.id == id)
        .map(|p| p.pinned_macros)
        .unwrap_or_default();
    let cfg = input.into_config(id, pinned)?;
    mgr.update(cfg.clone()).await?;
    Ok(cfg)
}

#[tauri::command]
pub async fn set_pinned_macros(mgr: Mgr<'_>, id: String, macros: Vec<String>) -> Result<(), String> {
    mgr.set_pinned_macros(&id, macros).await
}

#[tauri::command]
pub async fn remove_printer(mgr: Mgr<'_>, id: String) -> Result<(), String> {
    mgr.remove(&id).await
}

#[tauri::command]
pub async fn get_active_printer(mgr: Mgr<'_>) -> Result<Option<String>, String> {
    Ok(mgr.active().await)
}

#[tauri::command]
pub async fn set_active_printer(mgr: Mgr<'_>, id: Option<String>) -> Result<(), String> {
    mgr.set_active(id).await;
    Ok(())
}

/// `action`: `pause` | `resume` | `cancel` | `estop` | `firmware_restart` | `speed` (arg = percent)
/// | `gcode` (arg = script) | `preset` (arg = preset id) | `cooldown` | `reload_presets`
#[tauri::command]
pub async fn printer_action(
    mgr: Mgr<'_>,
    id: String,
    action: String,
    arg: Option<String>,
) -> Result<(), String> {
    let action = match action.as_str() {
        "pause" => Action::Pause,
        "resume" => Action::Resume,
        "cancel" => Action::Cancel,
        "estop" => Action::EmergencyStop,
        "firmware_restart" => Action::FirmwareRestart,
        "speed" => {
            let pct: u32 = arg
                .as_deref()
                .and_then(|s| s.trim().parse().ok())
                .ok_or_else(|| "Pourcentage invalide".to_string())?;
            Action::Gcode(format!("M220 S{}", pct.clamp(10, 500)))
        }
        "gcode" => {
            let script = arg.unwrap_or_default();
            if script.trim().is_empty() {
                return Err("Script vide".into());
            }
            Action::Gcode(script)
        }
        "preset" => Action::ApplyPreset(arg.unwrap_or_default()),
        "cooldown" => Action::Cooldown,
        "reload_presets" => Action::ReloadPresets,
        other => return Err(format!("Action inconnue: {other}")),
    };
    mgr.execute(&id, action).await
}

/// Connectivity check for the settings form. Returns a short description of the host.
#[tauri::command]
pub async fn test_connection(input: PrinterInput) -> Result<String, String> {
    let cfg = input.into_config("probe".into(), Vec::new())?;
    moonraker::probe(&cfg).await
}

#[tauri::command]
pub fn hide_popover(app: AppHandle) {
    tray::hide_popover(&app);
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}
