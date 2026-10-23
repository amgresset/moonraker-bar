// Mirrors src-tauri/src/moonraker/model.rs

export interface PrinterConfig {
  id: string;
  name: string;
  url: string;
  api_key?: string;
  camera_url?: string;
  web_url?: string;
  pinned_macros: string[];
}

export interface MacroInfo {
  name: string;
  description: string;
}

export interface PresetTarget {
  object: string;
  kind: "heater" | "temperature_fan";
  value: number;
}

export interface Preset {
  id: string;
  name: string;
  source: "mainsail" | "fluidd";
  targets: PresetTarget[];
  gcode: string;
}

export interface PrinterInput {
  name: string;
  url: string;
  api_key?: string;
  camera_url?: string;
  web_url?: string;
}

export type ConnectionState = "offline" | "connecting" | "connected";

export type PrintState =
  | ""
  | "standby"
  | "printing"
  | "paused"
  | "complete"
  | "cancelled"
  | "error";

export interface AfcLane {
  object: string;
  name: string;
  unit: string;
  hub: string;
  extruder: string;
  index: number;
  map: string;
  load: boolean;
  prep: boolean;
  tool_loaded: boolean;
  loaded_to_hub: boolean;
  material: string;
  color: string;
  filament_name: string;
  weight: number;
  spool_id: number | null;
  filament_status: "In Tool" | "Ready" | "Prep" | "Not Ready" | string;
  status: string;
  runout_lane: string;
  extruder_temp: number;
}

export interface AfcHub {
  object: string;
  name: string;
  state: boolean;
}

export interface AfcExtruder {
  object: string;
  name: string;
  lane_loaded: string;
  tool_start_status: boolean | null;
  tool_end_status: boolean | null;
}

export interface AfcState {
  current_load: string | null;
  current_lane: string | null;
  next_lane: string | null;
  current_state: string;
  error_state: boolean;
  bypass_state: boolean;
  current_toolchange: number;
  number_of_toolchanges: number;
  spoolman: boolean;
  message: string;
  message_type: string;
  units: string[];
  lanes: AfcLane[];
  hubs: AfcHub[];
  extruders: AfcExtruder[];
}

export interface Heater {
  temperature: number;
  target: number;
  power: number;
}

export interface Snapshot {
  connection: ConnectionState;
  klippy_state: string;
  klippy_message: string;
  print_state: PrintState;
  filename: string;
  progress: number;
  print_duration: number;
  total_duration: number;
  estimated_time: number | null;
  time_left: number | null;
  filament_used: number;
  current_layer: number | null;
  total_layer: number | null;
  extruder: Heater;
  bed: Heater;
  chamber: Heater | null;
  fan_speed: number;
  speed_factor: number;
  flow_factor: number;
  message: string;
  hostname: string;
  software_version: string;
  macros: MacroInfo[];
  presets: Preset[];
  cooldown_gcode: string;
  afc: AfcState | null;
  last_update: number;
  last_error: string | null;
}

export interface PrinterUpdate {
  id: string;
  snapshot: Snapshot;
}

export type PrinterAction =
  | "pause"
  | "resume"
  | "cancel"
  | "estop"
  | "firmware_restart"
  | "speed"
  | "gcode"
  | "preset"
  | "cooldown"
  | "reload_presets";

export const EMPTY_HEATER: Heater = { temperature: 0, target: 0, power: 0 };

export const EMPTY_SNAPSHOT: Snapshot = {
  connection: "offline",
  klippy_state: "",
  klippy_message: "",
  print_state: "",
  filename: "",
  progress: 0,
  print_duration: 0,
  total_duration: 0,
  estimated_time: null,
  time_left: null,
  filament_used: 0,
  current_layer: null,
  total_layer: null,
  extruder: EMPTY_HEATER,
  bed: EMPTY_HEATER,
  chamber: null,
  fan_speed: 0,
  speed_factor: 1,
  flow_factor: 1,
  message: "",
  hostname: "",
  software_version: "",
  macros: [],
  presets: [],
  cooldown_gcode: "",
  afc: null,
  last_update: 0,
  last_error: null,
};

export const isBusy = (s: Snapshot) =>
  s.print_state === "printing" || s.print_state === "paused";
