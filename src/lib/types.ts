// Mirrors src-tauri/src/moonraker/model.rs

export interface PrinterConfig {
  id: string;
  name: string;
  url: string;
  api_key?: string;
  camera_url?: string;
}

export interface PrinterInput {
  name: string;
  url: string;
  api_key?: string;
  camera_url?: string;
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
  | "gcode";

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
  last_update: 0,
  last_error: null,
};

export const isBusy = (s: Snapshot) =>
  s.print_state === "printing" || s.print_state === "paused";
