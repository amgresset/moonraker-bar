import type { Snapshot } from "./types";
import type { ArtState } from "@/components/PrinterArt";

export function artState(s: Snapshot): ArtState {
  if (s.connection !== "connected") return "offline";
  if (s.print_state === "error" || s.klippy_state === "shutdown" || s.klippy_state === "error") return "error";
  if (s.print_state === "printing") return "printing";
  if (s.print_state === "paused") return "paused";
  if (s.print_state === "complete") return "complete";
  return "idle";
}

/** Tool index from a Klipper extruder name: extruder -> 0, extruder2 -> 2. */
export function toolIndex(extruder: string): number {
  const n = Number(extruder.replace(/^extruder/, "") || 0);
  return Number.isFinite(n) ? n : 0;
}

/** Filament color per tool, for toolchangers: from the slicer, else the AFC lanes mapped to T0..T3. */
export function toolColors(s: Snapshot, count = 4): (string | null)[] {
  return Array.from({ length: count }, (_, i) => {
    const c = s.filament_colors[i];
    if (c) return c;
    const lane = s.afc?.lanes.find((l) => l.map === `T${i}`);
    return lane?.color || null;
  });
}

/** Color of the part being printed: the AFC lane in the tool, else the slicer's filament color. */
export function partColor(s: Snapshot): string | null {
  const lane = s.afc?.lanes.find((l) => l.name === s.afc?.current_load);
  if (lane?.color && lane.color !== "#000000") return lane.color;
  const tool = s.filament_colors[toolIndex(s.active_extruder)];
  return tool || s.filament_color;
}

/** Tool docks to draw, for toolchanger models only. */
export function artTools(model: string, s: Snapshot) {
  return model === "snapmaker_u1" ? { colors: toolColors(s, 4), active: toolIndex(s.active_extruder) } : undefined;
}
