import { useEffect } from "react";
import { Settings as SettingsIcon, X, Power } from "lucide-react";
import { useStore } from "./store";
import { backend } from "./lib/backend";
import { cn } from "./lib/format";
import { PrinterSwitcher } from "./components/PrinterSwitcher";
import { PrinterPanel } from "./components/PrinterPanel";
import { Settings } from "./components/Settings";
import { IconButton } from "./components/ui";

export default function App() {
  const { ready, view, setView, error, setError } = useStore();

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

  return (
    <div
      className={cn(
        "flex h-full flex-col overflow-hidden rounded-[var(--radius-popover)]",
        "border border-line bg-surface text-fg shadow-xl",
      )}
    >
      <header className="drag flex items-center gap-2 px-3 pt-2.5 pb-2">
        {view === "panel" ? (
          <PrinterSwitcher />
        ) : (
          <div className="flex-1 text-[15px] font-semibold tracking-tight">Réglages</div>
        )}
        {view === "panel" ? (
          <IconButton title="Réglages" onClick={() => setView("settings")}>
            <SettingsIcon size={15} />
          </IconButton>
        ) : (
          <IconButton title="Retour" onClick={() => setView("panel")}>
            <X size={15} />
          </IconButton>
        )}
      </header>

      <main className="min-h-0 flex-1 overflow-y-auto px-3 pb-3">
        {!ready ? (
          <div className="flex h-full items-center justify-center text-fg-2">Chargement…</div>
        ) : view === "panel" ? (
          <PrinterPanel />
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
