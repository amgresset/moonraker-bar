# Moonraker Bar

Un moniteur de barre de menus (macOS) / zone de notification (Linux, Windows) pour les imprimantes **Klipper** pilotées par **Moonraker**. Progression, temps restant, températures, couches, vitesse, caméra, commandes pause / reprise / annulation, plusieurs imprimantes, notifications. Réseau local uniquement : aucun compte, aucun cloud.

Construit avec **Tauri 2** (backend Rust) et **React + TypeScript + Tailwind** (interface).

```
┌─ barre de menus ──────────────────────┐
│  ◔ 64% · 1h02                          │   ← icône qui se remplit + texte (macOS)
└────────────────────────────────────────┘
        │ clic
        ▼
   ┌───────────────────────────┐
   │ ● Voron 2.4            ⚙  │
   │ ┌─────────────────────┐   │
   │ │ (◔ 64%)  benchy      │   │
   │ │  1h02    Fin 15:37   │   │
   │ │          Couche 84/131│  │
   │ └─────────────────────┘   │
   │  Buse 245°/245  Lit 105°  │
   │  [ Pause ]  [ Stop ]      │
   │  Vitesse 50 100 150 200   │
   │  ┌──── caméra ────┐       │
   └───────────────────────────┘
```

## Prérequis

- **Rust** stable (`rustup`), **Node 20+**.
- macOS : Xcode Command Line Tools (`xcode-select --install`).
- Linux : les dépendances Tauri (`webkit2gtk-4.1`, `libappindicator3`, etc.) — voir <https://v2.tauri.app/start/prerequisites/>.
- Une imprimante Klipper avec Moonraker joignable sur le réseau (`http://<hôte>:7125`).

## Démarrer

```bash
npm install
npm run icons        # génère src-tauri/icons/* depuis app-icon.svg (une seule fois)
npm run tauri dev    # lance l'app en mode développement
```

Au premier lancement l'icône apparaît dans la barre de menus ; clique dessus, puis **Ajouter une imprimante** : nom, URL Moonraker (`http://voron.local:7125`), clé API facultative, URL de snapshot caméra facultative (`/webcam/?action=snapshot`). **Tester** vérifie la connexion avant d'enregistrer.

Build de production :

```bash
npm run tauri build
```

Le binaire/.app/.dmg est dans `src-tauri/target/release/bundle/`.

## Fonctionnement

- **`src-tauri/src/moonraker/client.rs`** — un client websocket JSON-RPC par imprimante. Il s'abonne (`printer.objects.subscribe`) à `webhooks`, `print_stats`, `display_status`, `virtual_sdcard`, `extruder`, `heater_bed`, `fan`, `gcode_move` et au capteur de caisson s'il existe (`temperature_sensor chamber` / `heater_generic chamber`). Chaque delta est fusionné dans un `Snapshot`, le temps restant est calculé depuis `estimated_time` des métadonnées du fichier (`server.files.metadata`) ou extrapolé depuis la progression. Reconnexion automatique avec backoff (2 → 15 s), keepalive toutes les 20 s.
- **`manager.rs`** — liste des imprimantes (persistée via `tauri-plugin-store` dans `settings.json`), fan-out des mises à jour vers le front (événement `printer:update`), notifications sur les transitions d'état (terminé / erreur / annulé / pause / lancé), choix de l'imprimante suivie par la barre (épinglée, sinon celle qui imprime).
- **`tray.rs`** — l'icône est dessinée en RGBA à chaque changement (anneau de progression, barres de pause, point d'alerte) ; image *template* sur macOS pour suivre le thème clair/sombre. Le popover est une fenêtre sans décoration, positionnée sous l'icône par `tauri-plugin-positioner`, cachée quand elle perd le focus.
- **`src/`** — le front ne fait qu'afficher : `store.ts` (zustand) reçoit les événements Tauri, `components/PrinterPanel.tsx` est le panneau principal, `components/Settings.tsx` la gestion des imprimantes et le lancement au démarrage.

Commandes exposées au front : `list_printers`, `get_snapshots`, `add_printer`, `update_printer`, `remove_printer`, `get_active_printer`, `set_active_printer`, `printer_action` (`pause` | `resume` | `cancel` | `estop` | `firmware_restart` | `speed` | `gcode`), `test_connection`, `hide_popover`, `quit`.

## Moonraker

Si ton Mac n'est pas dans `trusted_clients` de `moonraker.conf`, crée une clé API dans Mainsail/Fluidd et renseigne-la dans le formulaire. Aucune modification de `cors_domains` n'est nécessaire : la connexion se fait en websocket depuis le backend Rust, pas depuis une page web.

## Feuille de route

- [ ] Widgets de bureau / Centre de notifications (macOS) — nécessite une extension Swift, hors Tauri.
- [ ] Spoolman : bobine en cours et filament restant.
- [ ] Macros Klipper en boutons configurables.
- [ ] Jog / homing depuis la barre.
- [ ] Historique des impressions.

## Licence

MIT.
