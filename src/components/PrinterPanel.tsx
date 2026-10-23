import { useEffect, useState, type ReactNode } from "react";
import { Pause, Play, Square, Gauge, AlertTriangle, RotateCcw, Layers, Clock, Flag, Fan, Camera } from "lucide-react";
import { useStore, useSelectedPrinter } from "@/store";
import { backend } from "@/lib/backend";
import { isBusy, type PrinterAction, type PrinterConfig, type Snapshot } from "@/lib/types";
import { cn, fmtDuration, fmtEta, fmtFilename, fmtPct, fmtTemp, fmtFilament } from "@/lib/format";
import { ProgressRing } from "./ProgressRing";
import { Badge, Button, Card } from "./ui";

export function PrinterPanel() {
  const { printer, snapshot } = useSelectedPrinter();
  const setView = useStore((s) => s.setView);

  if (!printer) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
        <div className="text-fg-2">Aucune imprimante configurée.</div>
        <Button variant="primary" onClick={() => setView("settings")}>
          Ajouter une imprimante
        </Button>
      </div>
    );
  }

  if (snapshot.connection !== "connected") {
    return <OfflineCard printer={printer} snapshot={snapshot} />;
  }

  return (
    <div className="flex flex-col gap-3">
      {snapshot.klippy_state !== "ready" && <KlippyBanner printer={printer} snapshot={snapshot} />}
      <JobCard snapshot={snapshot} />
      {snapshot.message && snapshot.message !== snapshot.filename && (
        <div className="truncate rounded-lg bg-surface-3 px-3 py-1.5 text-[12px] text-fg-2" title={snapshot.message}>
          {snapshot.message}
        </div>
      )}
      <Temperatures snapshot={snapshot} />
      <Controls printer={printer} snapshot={snapshot} />
      <CameraCard printer={printer} />
    </div>
  );
}

// ---------- pieces ----------

function OfflineCard({ printer, snapshot }: { printer: PrinterConfig; snapshot: Snapshot }) {
  const connecting = snapshot.connection === "connecting";
  return (
    <Card className="flex flex-col items-center gap-2 py-8 text-center">
      <div className={cn("h-3 w-3 rounded-full", connecting ? "bg-warn pulse-soft" : "bg-fg-3")} />
      <div className="font-semibold">{connecting ? "Connexion…" : "Hors ligne"}</div>
      <div className="text-[12px] text-fg-2">{printer.url}</div>
      {snapshot.last_error && <div className="text-[12px] text-danger">{snapshot.last_error}</div>}
      {!connecting && <div className="text-[11px] text-fg-3">Nouvelle tentative automatique…</div>}
    </Card>
  );
}

function KlippyBanner({ printer, snapshot }: { printer: PrinterConfig; snapshot: Snapshot }) {
  const [busy, setBusy] = useState(false);
  const bad = snapshot.klippy_state === "shutdown" || snapshot.klippy_state === "error";
  const restart = async () => {
    setBusy(true);
    try {
      await backend.action(printer.id, "firmware_restart");
    } catch (e) {
      useStore.getState().setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className={cn("rounded-lg px-3 py-2 text-[12px]", bad ? "bg-danger/10 text-danger" : "bg-warn/10 text-warn")}>
      <div className="flex items-center gap-1.5 font-medium">
        <AlertTriangle size={13} />
        Klipper : {labelKlippy(snapshot.klippy_state)}
      </div>
      {snapshot.klippy_message && (
        <div className="mt-1 max-h-16 overflow-y-auto whitespace-pre-wrap font-mono text-[11px] opacity-90">
          {snapshot.klippy_message}
        </div>
      )}
      {bad && (
        <Button size="sm" variant="danger" className="mt-2" icon={<RotateCcw size={12} />} disabled={busy} onClick={restart}>
          Firmware restart
        </Button>
      )}
    </div>
  );
}

function labelKlippy(state: string) {
  switch (state) {
    case "startup":
      return "démarrage";
    case "shutdown":
      return "arrêt (shutdown)";
    case "error":
      return "erreur";
    case "disconnected":
      return "déconnecté";
    default:
      return state || "inconnu";
  }
}

function JobCard({ snapshot: s }: { snapshot: Snapshot }) {
  const busy = isBusy(s);
  const paused = s.print_state === "paused";
  return (
    <Card>
      <div className="flex items-center gap-3">
        <ProgressRing value={busy ? s.progress : s.print_state === "complete" ? 1 : 0} paused={paused} size={104} stroke={9}>
          {busy ? (
            <>
              <div className="text-[22px] font-semibold leading-none tabular-nums tracking-tight">{fmtPct(s.progress)}</div>
              <div className="mt-1 text-[11px] text-fg-2">{s.time_left != null ? `${fmtDuration(s.time_left)} restant` : "estimation…"}</div>
            </>
          ) : (
            <div className="text-[13px] font-medium text-fg-2">{stateLabel(s.print_state)}</div>
          )}
        </ProgressRing>

        <div className="min-w-0 flex-1">
          <div className="mb-1">
            <StateBadge snapshot={s} />
          </div>
          <div className="truncate text-[14px] font-semibold tracking-tight" title={s.filename}>
            {s.filename ? fmtFilename(s.filename) : "Aucun fichier"}
          </div>
          {busy && (
            <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 text-[12px]">
              <Row icon={<Flag size={12} />} label="Fin" value={fmtEta(s.time_left)} />
              <Row icon={<Clock size={12} />} label="Écoulé" value={fmtDuration(s.print_duration)} />
              {s.total_layer != null && (
                <Row icon={<Layers size={12} />} label="Couche" value={`${s.current_layer ?? "–"} / ${s.total_layer}`} />
              )}
            </dl>
          )}
          {!busy && s.print_state === "complete" && (
            <div className="mt-1 text-[12px] text-fg-2">
              en {fmtDuration(s.print_duration)} · {fmtFilament(s.filament_used)} de filament
            </div>
          )}
        </div>
      </div>
    </Card>
  );
}

function Row({ icon, label, value }: { icon: ReactNode; label: string; value: string }) {
  return (
    <>
      <dt className="flex items-center gap-1 text-fg-3">
        {icon}
        {label}
      </dt>
      <dd className="truncate text-right tabular-nums text-fg">{value}</dd>
    </>
  );
}

function stateLabel(state: string) {
  switch (state) {
    case "printing":
      return "Impression";
    case "paused":
      return "En pause";
    case "complete":
      return "Terminé";
    case "cancelled":
      return "Annulé";
    case "error":
      return "Erreur";
    case "standby":
    case "":
      return "Prête";
    default:
      return state;
  }
}

function StateBadge({ snapshot: s }: { snapshot: Snapshot }) {
  const tone =
    s.print_state === "printing" ? "ok" : s.print_state === "paused" ? "warn" : s.print_state === "error" ? "danger" : s.print_state === "complete" ? "accent" : "neutral";
  return <Badge tone={tone}>{stateLabel(s.print_state)}</Badge>;
}

function Temperatures({ snapshot: s }: { snapshot: Snapshot }) {
  const tiles = [
    { label: "Buse", h: s.extruder, color: "text-heat" },
    { label: "Lit", h: s.bed, color: "text-bed" },
    ...(s.chamber ? [{ label: "Caisson", h: s.chamber, color: "text-chamber" }] : []),
  ];
  return (
    <div className={cn("grid gap-2", tiles.length === 3 ? "grid-cols-3" : "grid-cols-2")}>
      {tiles.map((t) => (
        <Card key={t.label} className="px-3 py-2">
          <div className="text-[11px] font-medium text-fg-2">{t.label}</div>
          <div className="flex items-baseline gap-1">
            <span className={cn("text-[18px] font-semibold tabular-nums tracking-tight", t.h.target > 0 ? t.color : "")}>{fmtTemp(t.h.temperature)}</span>
            {t.h.target > 0 && <span className="text-[11px] tabular-nums text-fg-3">/ {fmtTemp(t.h.target)}</span>}
          </div>
          {t.h.target > 0 && (
            <div className="mt-1 h-1 overflow-hidden rounded-full bg-fg/10">
              <div
                className={cn("h-full rounded-full bg-current transition-[width] duration-500", t.color)}
                style={{ width: `${Math.min(100, (t.h.temperature / t.h.target) * 100)}%` }}
              />
            </div>
          )}
        </Card>
      ))}
    </div>
  );
}

const SPEEDS = [50, 100, 150, 200];

function Controls({ printer, snapshot: s }: { printer: PrinterConfig; snapshot: Snapshot }) {
  const [pending, setPending] = useState<string | null>(null);
  const busy = isBusy(s);
  const ready = s.klippy_state === "ready";

  const run = async (action: PrinterAction, arg?: string) => {
    setPending(action);
    try {
      await backend.action(printer.id, action, arg);
    } catch (e) {
      useStore.getState().setError(String(e));
    } finally {
      setPending(null);
    }
  };

  return (
    <Card className="flex flex-col gap-2.5">
      <div className="flex items-center gap-2">
        {s.print_state === "paused" ? (
          <Button variant="primary" className="flex-1" icon={<Play size={14} />} disabled={!ready || pending != null} onClick={() => run("resume")}>
            Reprendre
          </Button>
        ) : (
          <Button variant="primary" className="flex-1" icon={<Pause size={14} />} disabled={!busy || !ready || pending != null} onClick={() => run("pause")}>
            Pause
          </Button>
        )}
        <ConfirmButton
          label="Stop"
          confirmLabel="Annuler ?"
          icon={<Square size={14} />}
          disabled={!busy || !ready || pending != null}
          onConfirm={() => run("cancel")}
        />
      </div>

      <div className="flex items-center gap-2 text-[12px]">
        <span className="flex items-center gap-1 text-fg-2">
          <Gauge size={13} /> Vitesse
        </span>
        <div className="flex flex-1 gap-1">
          {SPEEDS.map((pct) => {
            const active = Math.round(s.speed_factor * 100) === pct;
            return (
              <button
                key={pct}
                disabled={!ready || pending != null}
                onClick={() => run("speed", String(pct))}
                className={cn(
                  "flex-1 rounded-md py-1 text-[12px] font-medium tabular-nums transition disabled:opacity-40",
                  active ? "bg-accent text-accent-fg" : "bg-surface-3 text-fg-2 hover:text-fg",
                )}
              >
                {pct}%
              </button>
            );
          })}
        </div>
        <span className="flex items-center gap-1 text-fg-3" title="Ventilateur pièce">
          <Fan size={13} className={cn(s.fan_speed > 0 && "animate-spin [animation-duration:2s]")} /> {Math.round(s.fan_speed * 100)}%
        </span>
      </div>

      <div className="flex justify-end">
        <ConfirmButton
          label="Arrêt d'urgence"
          confirmLabel="Confirmer l'arrêt ?"
          variant="ghost"
          size="sm"
          icon={<AlertTriangle size={12} />}
          disabled={pending != null}
          onConfirm={() => run("estop")}
          className="text-danger hover:bg-danger/10 hover:text-danger"
        />
      </div>
    </Card>
  );
}

/** Two-step button: first click arms it for 3 s, second click fires. */
function ConfirmButton({
  label,
  confirmLabel,
  onConfirm,
  variant = "danger",
  size = "md",
  icon,
  disabled,
  className,
}: {
  label: string;
  confirmLabel: string;
  onConfirm: () => void;
  variant?: "danger" | "ghost";
  size?: "sm" | "md";
  icon?: ReactNode;
  disabled?: boolean;
  className?: string;
}) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const t = setTimeout(() => setArmed(false), 3000);
    return () => clearTimeout(t);
  }, [armed]);
  return (
    <Button
      variant={armed ? "danger" : variant}
      size={size}
      icon={icon}
      disabled={disabled}
      className={cn(armed && "ring-2 ring-danger/40", className)}
      onClick={() => {
        if (armed) {
          setArmed(false);
          onConfirm();
        } else {
          setArmed(true);
        }
      }}
    >
      {armed ? confirmLabel : label}
    </Button>
  );
}

function CameraCard({ printer }: { printer: PrinterConfig }) {
  const raw = printer.camera_url?.trim();
  const [tick, setTick] = useState(0);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!raw) return;
    const t = setInterval(() => setTick((n) => n + 1), 4000);
    return () => clearInterval(t);
  }, [raw]);
  if (!raw) return null;
  const abs = raw.startsWith("http") ? raw : new URL(raw, printer.url.endsWith("/") ? printer.url : printer.url + "/").toString();
  const src = `${abs}${abs.includes("?") ? "&" : "?"}_t=${tick}`;
  return (
    <Card className="overflow-hidden p-0">
      {failed ? (
        <div className="flex items-center justify-center gap-2 py-6 text-[12px] text-fg-3">
          <Camera size={14} /> Caméra indisponible
        </div>
      ) : (
        <img src={src} alt="Caméra" className="block aspect-video w-full object-cover" onError={() => setFailed(true)} onLoad={() => setFailed(false)} />
      )}
    </Card>
  );
}
