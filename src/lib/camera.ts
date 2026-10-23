import type { PrinterConfig, Snapshot, Webcam } from "./types";
import { webUiUrl } from "./format";

export interface CameraSource {
  name: string;
  /** MJPEG stream, shown live in an <img>. Null when the camera only offers snapshots. */
  stream: string | null;
  /** Single JPEG, polled as a fallback. */
  snapshot: string | null;
  /** CSS transform for the camera's flip / rotation settings. */
  transform: string;
}

/** Services whose stream_url is an MJPEG stream an <img> can play. */
const MJPEG_SERVICES = ["mjpegstreamer", "mjpegstreamer-adaptive", "uv4l-mjpeg"];

/**
 * Where to read the camera from: the URL typed in the printer settings if any,
 * else the first enabled webcam configured in Moonraker (Mainsail / Fluidd).
 * Relative URLs are resolved against the web UI host, where nginx serves /webcam.
 */
export function cameraSource(printer: PrinterConfig, snapshot: Snapshot): CameraSource | null {
  const base = webUiUrl(printer) ?? printer.url;
  const resolve = (u: string) => {
    if (!u) return null;
    try {
      return new URL(u, base.endsWith("/") ? base : base + "/").toString();
    } catch {
      return null;
    }
  };

  const manual = printer.camera_url?.trim();
  if (manual) {
    if (manual.includes("action=snapshot")) {
      return { name: "Camera", snapshot: resolve(manual), stream: resolve(manual.replace("action=snapshot", "action=stream")), transform: "" };
    }
    if (manual.includes("action=stream")) {
      return { name: "Camera", stream: resolve(manual), snapshot: resolve(manual.replace("action=stream", "action=snapshot")), transform: "" };
    }
    return { name: "Camera", stream: null, snapshot: resolve(manual), transform: "" };
  }

  const cam = snapshot.webcams.find((w) => w.enabled) ?? null;
  if (!cam) return null;
  const isMjpeg = MJPEG_SERVICES.includes(cam.service) || cam.stream_url.includes("action=stream");
  const src: CameraSource = {
    name: cam.name || "Camera",
    stream: isMjpeg ? resolve(cam.stream_url) : null,
    snapshot: resolve(cam.snapshot_url),
    transform: transformFor(cam),
  };
  return src.stream || src.snapshot ? src : null;
}

function transformFor(cam: Webcam): string {
  const parts: string[] = [];
  if (cam.rotation) parts.push(`rotate(${cam.rotation}deg)`);
  if (cam.flip_horizontal) parts.push("scaleX(-1)");
  if (cam.flip_vertical) parts.push("scaleY(-1)");
  return parts.join(" ");
}
