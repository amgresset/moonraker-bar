import { ChevronRight, Pin } from "lucide-react";
import { useStore } from "@/store";
import { EMPTY_SNAPSHOT, isBusy, type PrinterConfig, type Snapshot } from "@/lib/types";
import { cn, fmtDuration, fmtFilename, fmtPct, fmtTemp } from "@/lib/format";
import { ProgressRing } from "./ProgressRing";
import { Card } from "./ui";

/** All printers at a glance; click one to open its panel. */
export function Fleet() {
  const { printers, snapshots, activeId, select, pin } = useStore();
  return (
    <div className="flex flex-col gap-2">
      {printers.map((p) => (
        <FleetRow
          key={p.id}
          printer={p}
          snapshot={snapshots[p.id] ?? EMPTY_SNAPSHOT}
          pinned={activeId === p.id}
          onOpen={() => select(p.id)}
          onPin={() => pin(activeId === p.id ? null : p.id)}
        />
      ))}
    </div>
  );
}

function FleetRow({
  printer,
  snapshot: s,
  pinned,
  onOpen,
  onPin,
}: {
  printer: PrinterConfig;
  snapshot: Snapshot;
  pinned: boolean;
  onOpen: () => void;
  onPin: () => void;
}) {
  const online = s.connection === "connected";
  const busy = isBusy(s);
  const paused = s.print_state === "paused";
  const alert = s.print_state === "error" || s.klippy_state === "shutdown" || s.klippy_state === "error";

  const subtitle = !online
    ? s.connection === "connecting"
      ? "Connecting…"
      : "Offline"
    : alert
      ? "Klipper error"
      : busy
        ? fmtFilename(s.filename)
        : s.print_state === "complete"
          ? `Done · ${fmtFilename(s.filename)}`
          : "Ready";

  const detail = online && busy ? (s.time_left != null ? `${fmtDuration(s.time_left)} left` : "estimating…") : online ? `${fmtTemp(s.extruder.temperature)} · ${fmtTemp(s.bed.temperature)}` : "";

  return (
    <Card className={cn("group flex items-center gap-3 p-2.5 transition hover:bg-surface-3/50", !online && "opacity-70")}>
      <button onClick={onOpen} className="flex min-w-0 flex-1 items-center gap-3 text-left">
        <ProgressRing value={busy ? s.progress : 0} paused={paused} size={44} stroke={5} className={cn(!busy && "opacity-60")}>
          {busy ? (
            <span className="text-[10px] font-semibold tabular-nums">{fmtPct(s.progress)}</span>
          ) : (
            <span className={cn("h-2 w-2 rounded-full", !online ? "bg-fg-3" : alert ? "bg-danger" : "bg-ok")} />
          )}
        </ProgressRing>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span className="truncate text-[13px] font-semibold tracking-tight">{printer.name}</span>
            {paused && <span className="rounded-full bg-warn/15 px-1.5 text-[10px] font-medium text-warn">paused</span>}
          </div>
          <div className={cn("truncate text-[12px]", alert ? "text-danger" : "text-fg-2")}>{subtitle}</div>
          {detail && <div className="truncate text-[11px] tabular-nums text-fg-3">{detail}</div>}
        </div>
      </button>
      <button
        title={pinned ? "The menu bar follows this printer" : "Pin to the menu bar"}
        onClick={onPin}
        className={cn(
          "inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-md transition",
          pinned ? "bg-accent/15 text-accent" : "text-fg-3 opacity-0 hover:bg-surface-3 hover:text-fg group-hover:opacity-100",
        )}
      >
        <Pin size={12} />
      </button>
      <ChevronRight size={14} className="shrink-0 text-fg-3" />
    </Card>
  );
}
