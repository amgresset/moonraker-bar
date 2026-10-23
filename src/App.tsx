import { useEffect, useLayoutEffect, useRef } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { Settings as SettingsIcon, X, Power, ChevronLeft } from "lucide-react";
import { useStore } from "./store";
import { backend } from "./lib/backend";
import { cn } from "./lib/format";
import { PrinterSwitcher } from "./components/PrinterSwitcher";
import { PrinterPanel } from "./components/PrinterPanel";
import { Fleet } from "./components/Fleet";
import { Settings } from "./components/Settings";
import { IconButton } from "./components/ui";

const WIDTH = 360;
const MIN_HEIGHT = 220;
const MAX_HEIGHT = 700;

export default function App() {
  const { ready, view, setView, error, setError, printers } = useStore();
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    useStore.getState().init().catch((e) => setError(String(e)));
  }, [setError]);

  // Escape closes the popover, like a native menu.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") backend.hidePopover();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Size the native window to the content, so the popover never shows dead space.
  useLayoutEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    let last = 0;
    const apply = () => {
      const h = Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, Math.ceil(el.getBoundingClientRect().height)));
      if (h === last) return;
      last = h;
      getCurrentWindow()
        .setSize(new LogicalSize(WIDTH, h))
        .catch(() => {});
    };
    const ro = new ResizeObserver(apply);
    ro.observe(el);
    apply();
    return () => ro.disconnect();
  }, []);

  const multi = printers.length > 1;
  const title = view === "settings" ? "Réglages" : view === "fleet" ? "Imprimantes" : null;

  return (
    <div
      ref={rootRef}
      className={cn(
        "flex flex-col overflow-hidden rounded-[var(--radius-popover)]",
        "border border-line bg-surface text-fg",
      )}
      style={{ maxHeight: MAX_HEIGHT }}
    >
      <header className="drag flex items-center gap-1 px-3 pt-2.5 pb-2">
        {view === "panel" ? (
          <>
            {multi && (
              <IconButton title="Toutes les imprimantes" onClick={() => setView("fleet")} className="-ml-1 mr-0.5">
                <ChevronLeft size={15} />
              </IconButton>
            )}
            <PrinterSwitcher />
          </>
        ) : (
          <div className="flex-1 text-[15px] font-semibold tracking-tight">{title}</div>
        )}
        {view === "settings" ? (
          <IconButton title="Retour" onClick={() => setView(multi ? "fleet" : "panel")}>
            <X size={15} />
          </IconButton>
        ) : (
          <IconButton title="Réglages" onClick={() => setView("settings")}>
            <SettingsIcon size={15} />
          </IconButton>
        )}
      </header>

      <main className="min-h-0 flex-1 overflow-y-auto px-3 pb-3">
        {!ready ? (
          <div className="py-10 text-center text-fg-2">Chargement…</div>
        ) : view === "panel" ? (
          <PrinterPanel />
        ) : view === "fleet" ? (
          <Fleet />
        ) : (
          <Settings />
        )}
      </main>

      {error && (
        <div className="mx-3 mb-2 rounded-lg bg-danger/10 px-3 py-2 text-[12px] text-danger">
          {error}
          <button className="ml-2 underline" onClick={() => setError(null)}>
            OK
          </button>
        </div>
      )}

      <footer className="flex items-center justify-between border-t border-line px-3 py-1.5 text-[11px] text-fg-3">
        <span>Moonraker Bar</span>
        <button
          className="flex items-center gap-1 rounded px-1.5 py-0.5 hover:bg-surface-3 hover:text-fg"
          onClick={() => backend.quit()}
          title="Quitter"
        >
          <Power size={11} /> Quitter
        </button>
      </footer>
    </div>
  );
}
