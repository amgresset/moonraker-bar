import { useCallback, useEffect, useRef, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

const CHECK_EVERY_MS = 6 * 60 * 60 * 1000;

export type UpdaterState =
  | { kind: "idle" }
  | { kind: "available"; version: string; notes: string }
  | { kind: "downloading"; version: string; progress: number | null }
  | { kind: "ready"; version: string }
  | { kind: "error"; message: string };

/** Checks GitHub Releases at startup and every 6 h; install on demand, then relaunch. */
export function useUpdater() {
  const [state, setState] = useState<UpdaterState>({ kind: "idle" });
  const update = useRef<Update | null>(null);

  const runCheck = useCallback(async () => {
    try {
      const u = await check();
      if (u) {
        update.current = u;
        setState({ kind: "available", version: u.version, notes: u.body ?? "" });
      }
    } catch (e) {
      // Offline, no release yet, or dev build: stay quiet, try again later.
      console.warn("update check failed", e);
    }
  }, []);

  useEffect(() => {
    if (import.meta.env.DEV) return;
    runCheck();
    const t = setInterval(runCheck, CHECK_EVERY_MS);
    return () => clearInterval(t);
  }, [runCheck]);

  const install = useCallback(async () => {
    const u = update.current;
    if (!u) return;
    let total = 0;
    let done = 0;
    setState({ kind: "downloading", version: u.version, progress: null });
    try {
      await u.downloadAndInstall((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        else if (ev.event === "Progress") {
          done += ev.data.chunkLength;
          setState({ kind: "downloading", version: u.version, progress: total ? done / total : null });
        } else if (ev.event === "Finished") setState({ kind: "ready", version: u.version });
      });
      await relaunch();
    } catch (e) {
      setState({ kind: "error", message: String(e) });
    }
  }, []);

  return { state, install, check: runCheck };
}
