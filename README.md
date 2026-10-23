# Moonraker Bar

Cross-platform menu bar (macOS) / system tray (Linux, Windows) app for **Klipper** printers driven by **Moonraker**. Live progress, time left, temperatures, layers, print speed, camera, pause / resume / cancel, your Klipper macros, Mainsail / Fluidd preheat presets, AFC (Box Turtle) lanes, several printers, notifications. Local network only: no account, no cloud, no telemetry.

Built with **Tauri 2** (Rust backend) and **React + TypeScript + Tailwind** (UI).

```
┌─ menu bar ─────────────────────────────┐
│  ◔ 64% · 1h02                           │   ← icon fills with progress + text (macOS)
└─────────────────────────────────────────┘
        │ click
        ▼
   ┌────────────────────────────┐
   │ ‹ ● Voron 2.4        ↗ ⚙   │
   │ ┌──────────────────────┐   │
   │ │ (◔ 64%)  voron_skirt │   │
   │ │  1h02    ETA 17:42   │   │
   │ │          Layer 84/131│   │
   │ └──────────────────────┘   │
   │  Nozzle 245°/250 Bed 105°  │
   │  [ Pause ]  [ Cancel ]     │
   │  Speed 50 100 150 200      │
   │  AFC  T0 T1 T2 T3          │
   │  Macros  Light on  …       │
   │  [ Home ] [ Motors ] [STOP]│
   └────────────────────────────┘
```

## Features

- **Menu bar icon** drawn live: a ring that fills with progress, pause bars while paused, a dot when Klipper is in error; template image on macOS so it follows the light/dark menu bar. Progress and time left as text beside it (macOS).
- **Popover** anchored under the icon, sized to its content (no scrollbar as long as it fits on screen), closed on blur or Escape.
- **Job card**: progress, time left, ETA as clock time, elapsed, layers, filename, last print summary when idle.
- **Temperatures**: every heater, sensor and temperature fan Klipper reports; pick which tiles to show per printer in Settings.
- **Preheat presets** read from Mainsail (`mainsail/presets`) and Fluidd (`fluidd/uiSettings`) in the Moonraker database, applied the way Mainsail does (`SET_HEATER_TEMPERATURE`, `SET_TEMPERATURE_FAN_TARGET`, then the preset's G-code), plus a Cool down button.
- **Print controls** while printing: pause / resume, cancel (two-step), print speed 50–200 %, part fan readout.
- **Quick actions**: Home (`G28`), Motors off (`M84`), emergency stop (two-step, always available), firmware restart when Klipper is shut down.
- **Macros**: every public `gcode_macro` with its description; pin the ones you want as buttons, per printer.
- **AFC** (Armored Turtle Automated Filament Control): detected automatically. Lanes with T-number, filament color, material and remaining weight; lane in the tool, hub state, tool-change counter during a print, load a lane (two-step), `TOOL_UNLOAD`, `AFC_RESUME` on error.
- **Camera** snapshot refreshed every 4 s.
- **Several printers**: a fleet view, a switcher in the header, and a pin to choose which printer the menu bar follows (default: whichever is printing).
- **Notifications**: print started, paused, finished, cancelled, failed.
- **Open in Mainsail / Fluidd** from the header.
- **Layout**: reorder and hide the panel's cards in Settings (drag or arrows).
- Launch at login.

## Requirements

- **Rust** stable (`rustup`) and **Node 20+**.
- macOS: Xcode Command Line Tools (`xcode-select --install`).
- Linux: the Tauri prerequisites (`webkit2gtk-4.1`, `libappindicator3`, …) — see <https://v2.tauri.app/start/prerequisites/>.
- A Klipper printer with Moonraker reachable on the network (`http://<host>:7125`).

## Getting started

```bash
npm install
npm run tauri dev
```

The icon appears in the menu bar. Click it, then **Add a printer**: name, Moonraker URL (`http://voron.local:7125`), optional API key, optional web UI URL (defaults to the Moonraker host on port 80), optional camera snapshot URL (`/webcam/?action=snapshot`). **Test** checks the connection before saving.

Production build:

```bash
npm run tauri build
```

The bundle (`.app` / `.dmg`, `.deb` / `.AppImage`, `.msi`) lands in `src-tauri/target/release/bundle/`. Icons are generated from `src-tauri/icons/app-icon.svg` with `npm run icons`.

## How it works

- **`src-tauri/src/moonraker/client.rs`** — one websocket JSON-RPC client per printer. On connect it lists printer objects, subscribes (`printer.objects.subscribe`) to `webhooks`, `print_stats`, `display_status`, `virtual_sdcard`, `gcode_move`, `fan`, every temperature object, and the AFC objects when present. Each delta is merged into a `Snapshot`; time left comes from the file's `estimated_time` (`server.files.metadata`) or is extrapolated from progress. Commands run in their own task (G-code waits up to 15 min, since Klipper only answers once the script has finished). Automatic reconnect with backoff (2 → 15 s), keepalive every 20 s.
- **`manager.rs`** — printer list, layout and preferences persisted with `tauri-plugin-store` (`settings.json`), fan-out of updates to the UI (`printer:update` event), notifications on print-state transitions, choice of the printer the tray follows.
- **`tray.rs`** — the icon is rendered as RGBA on every change; the popover is an undecorated window positioned under the icon by `tauri-plugin-positioner` and hidden when it loses focus.
- **`src/`** — the front only displays and sends commands: `store.ts` (zustand) receives the Tauri events, `components/PrinterPanel.tsx` is the main panel, `components/Afc.tsx` the AFC card, `components/Fleet.tsx` the fleet view, `components/Settings.tsx` printers, temperature tiles, layout and autostart.

Commands exposed to the front: `list_printers`, `get_snapshots`, `add_printer`, `update_printer`, `remove_printer`, `set_pinned_macros`, `set_shown_temps`, `get_layout`, `set_layout`, `get_active_printer`, `set_active_printer`, `printer_action` (`pause` | `resume` | `cancel` | `estop` | `firmware_restart` | `speed` | `gcode` | `preset` | `cooldown` | `reload_presets`), `test_connection`, `hide_popover`, `quit`.

## Moonraker notes

- If the machine running Moonraker Bar is not in `trusted_clients` of `moonraker.conf`, create an API key in Mainsail / Fluidd and paste it in the printer form. No `cors_domains` change is needed: the connection is a websocket from the Rust backend, not from a web page.
- Presets live in the Moonraker database, so create them in Mainsail or Fluidd; they are read on connect.
- Settings are stored in the OS app-data folder (macOS: `~/Library/Application Support/dev.modship.moonraker-bar/settings.json`).

## Roadmap

- [ ] Desktop widgets / Notification Center (macOS) — needs a Swift extension outside Tauri.
- [ ] Spoolman: active spool, remaining filament.
- [ ] G-code file browser: start a print from the recent files.
- [ ] Print history.
- [ ] Signed, notarized builds and auto-update via GitHub Releases.

## License

MIT.
