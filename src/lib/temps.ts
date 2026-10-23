import type { PrinterConfig, Snapshot, TempReading } from "./types";

/** Display name for a Klipper temperature object. */
export function tempLabel(object: string): string {
  if (object === "extruder") return "Nozzle";
  if (/^extruder\d+$/.test(object)) return `Nozzle ${Number(object.slice(8)) + 1}`;
  if (object === "heater_bed") return "Bed";
  const name = object.split(" ").slice(1).join(" ") || object;
  if (/chamber/i.test(name)) return "Chamber";
  return name.replace(/[_-]+/g, " ").replace(/^\w/, (c) => c.toUpperCase());
}

/** Tailwind text color class for a reading. */
export function tempColor(t: TempReading): string {
  if (t.kind === "extruder") return "text-heat";
  if (t.kind === "heater_bed") return "text-bed";
  if (/chamber/i.test(t.object)) return "text-chamber";
  if (t.kind === "heater_generic") return "text-warn";
  if (t.kind === "temperature_fan") return "text-chamber";
  return "text-fg-2";
}

/** Default tiles when the user has not chosen any: extruders, bed, anything chamber-like. */
export function defaultTemps(all: TempReading[]): string[] {
  return all
    .filter((t) => t.kind === "extruder" || t.kind === "heater_bed" || /chamber/i.test(t.object))
    .map((t) => t.object);
}

/** Readings to display for a printer, in the configured order. */
export function visibleTemps(printer: PrinterConfig, snapshot: Snapshot): TempReading[] {
  const all = snapshot.temperatures;
  if (all.length === 0) {
    // Not yet discovered: fall back to the fixed fields so the panel is never empty.
    const fixed: TempReading[] = [
      { object: "extruder", kind: "extruder", temperature: snapshot.extruder.temperature, target: snapshot.extruder.target, power: snapshot.extruder.power, speed: null },
      { object: "heater_bed", kind: "heater_bed", temperature: snapshot.bed.temperature, target: snapshot.bed.target, power: snapshot.bed.power, speed: null },
    ];
    if (snapshot.chamber) {
      fixed.push({ object: "heater_generic chamber", kind: "heater_generic", temperature: snapshot.chamber.temperature, target: snapshot.chamber.target, power: null, speed: null });
    }
    return fixed;
  }
  const wanted = printer.shown_temps.length ? printer.shown_temps : defaultTemps(all);
  const byObject = new Map(all.map((t) => [t.object, t]));
  return wanted.map((o) => byObject.get(o)).filter((t): t is TempReading => t != null);
}
