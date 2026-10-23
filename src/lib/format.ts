import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export const cn = (...inputs: ClassValue[]) => twMerge(clsx(inputs));

/** 3725 → "1h02", 300 → "5 min", 42 → "<1 min" */
export function fmtDuration(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds)) return "—";
  const total = Math.max(0, Math.round(seconds));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  if (h > 0) return `${h}h${String(m).padStart(2, "0")}`;
  if (m > 0) return `${m} min`;
  return "<1 min";
}

/** Clock time `seconds` from now, e.g. "14:37" (or "tomorrow 08:12"). */
export function fmtEta(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds)) return "—";
  const eta = new Date(Date.now() + seconds * 1000);
  const time = eta.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const today = new Date();
  const sameDay = eta.toDateString() === today.toDateString();
  if (sameDay) return time;
  const tomorrow = new Date(today);
  tomorrow.setDate(today.getDate() + 1);
  if (eta.toDateString() === tomorrow.toDateString()) return `tomorrow ${time}`;
  return eta.toLocaleDateString([], { weekday: "short" }) + " " + time;
}

export const fmtTemp = (t: number) => `${Math.round(t)}°`;

export const fmtPct = (ratio: number) => `${Math.round(ratio * 100)}%`;

/** Strip directories and the .gcode extension. */
export function fmtFilename(path: string): string {
  const base = path.split("/").pop() ?? path;
  return base.replace(/\.gcode$/i, "");
}

export function fmtFilament(mm: number): string {
  if (mm <= 0) return "—";
  const m = mm / 1000;
  return m >= 10 ? `${m.toFixed(1)} m` : `${m.toFixed(2)} m`;
}

/** Where the printer's web UI lives: the configured URL, else the Moonraker host on port 80. */
export function webUiUrl(printer: { url: string; web_url?: string }): string | null {
  const explicit = printer.web_url?.trim();
  if (explicit) return explicit.startsWith("http") ? explicit : `http://${explicit}`;
  try {
    const u = new URL(printer.url);
    return `${u.protocol}//${u.hostname}/`;
  } catch {
    return null;
  }
}
