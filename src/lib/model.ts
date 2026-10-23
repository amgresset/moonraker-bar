import type { PrinterConfig, Snapshot } from "./types";

export const MODELS = ["voron24", "trident", "v0", "snapmaker_u1", "bedslinger", "generic"] as const;
export type Model = (typeof MODELS)[number];

export const MODEL_LABELS: Record<Model, string> = {
  voron24: "Voron 2.4",
  trident: "Voron Trident",
  v0: "Voron V0 / small CoreXY",
  snapmaker_u1: "Snapmaker U1",
  bedslinger: "Bed slinger",
  generic: "Other enclosed CoreXY",
};

/**
 * Which illustration to draw: the user's choice, else a guess from the printer's
 * name, else from its Klipper config (kinematics, gantry levelling, bed size).
 */
export function detectModel(printer: PrinterConfig, s: Snapshot): Model {
  if (printer.model && (MODELS as readonly string[]).includes(printer.model)) return printer.model as Model;
  return guessModel(printer, s);
}

export function guessModel(printer: PrinterConfig, s: Snapshot): Model {
  const name = `${printer.name} ${s.hostname}`.toLowerCase();
  if (/snapmaker|\bu1\b/.test(name)) return "snapmaker_u1";
  if (/\bv0\b|v0\.|zero|micron|salad/.test(name)) return "v0";
  if (/trident/.test(name)) return "trident";
  if (/2\.4|\bv2\b|voron ?2/.test(name)) return "voron24";
  if (/switchwire|ender|prusa|\bmk[34]|cr-?10|neptune|sovol|bed ?slinger/.test(name)) return "bedslinger";

  const m = s.machine;
  // Four extruders on a 270 mm CoreXY: the U1's toolchanger.
  const extruders = s.temperatures.filter((t) => t.kind === "extruder").length;
  if (extruders >= 4 && m.kinematics === "corexy" && Math.round(m.x_max) >= 265 && Math.round(m.x_max) <= 275) return "snapmaker_u1";
  if (m.kinematics === "cartesian") return "bedslinger";
  if (m.leveling === "qgl") return m.x_max > 0 && m.x_max <= 150 ? "v0" : "voron24";
  if (m.x_max > 0 && m.x_max <= 150) return "v0";
  if (m.leveling === "z_tilt") return "trident";
  return "generic";
}
