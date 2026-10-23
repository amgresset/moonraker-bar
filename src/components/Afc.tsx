import { useEffect, useState } from "react";
import { Turtle, AlertTriangle, Loader2, ArrowDownToLine, ArrowUpFromLine, RotateCcw } from "lucide-react";
import { isBusy, type AfcLane, type AfcState, type PrinterConfig, type Snapshot } from "@/lib/types";
import { useAction } from "@/lib/useAction";
import { cn } from "@/lib/format";
import { Card } from "./ui";

/** Armored Turtle AFC: lanes with their spools, what's in the tool, hub state and errors. */
export function AfcCard({ printer, snapshot: s, afc }: { printer: PrinterConfig; snapshot: Snapshot; afc: AfcState }) {
  const { pending, run } = useAction(printer.id);
  const [armed, setArmed] = useState<string | null>(null);
  useEffect(() => {
    if (!armed) return;
    const t = setTimeout(() => setArmed(null), 3000);
    return () => clearTimeout(t);
  }, [armed]);

  const ready = s.klippy_state === "ready";
  const printing = isBusy(s);
  const working = afc.current_state !== "Idle" && afc.current_state !== "Initialized";
  const locked = !ready || printing || working || pending != null;
  const loaded = afc.lanes.find((l) => l.name === afc.current_load) ?? null;
  const hubLoaded = afc.hubs.some((h) => h.state);

  const change = (lane: AfcLane) => {
    const cmd = lane.map || `CHANGE_TOOL LANE=${lane.name}`;
    if (armed === lane.name) {
      setArmed(null);
      run("gcode", cmd);
    } else {
      setArmed(lane.name);
    }
  };

  const byUnit = new Map<string, AfcLane[]>();
  for (const l of afc.lanes) {
    const list = byUnit.get(l.unit) ?? [];
    list.push(l);
    byUnit.set(l.unit, list);
  }

  return (
    <Card className="flex flex-col gap-2 p-2.5">
      <div className="flex items-center justify-between">
        <span className="flex items-center gap-1 text-[11px] font-medium uppercase tracking-wide text-fg-2">
          <Turtle size={12} /> AFC
          {afc.bypass_state && <span className="ml-1 rounded-full bg-warn/15 px-1.5 text-[10px] normal-case tracking-normal text-warn">bypass</span>}
        </span>
        <span className="flex items-center gap-2 text-[11px] text-fg-3">
          {printing && afc.number_of_toolchanges > 0 && (
            <span className="tabular-nums" title="Tool changes">
              {afc.current_toolchange}/{afc.number_of_toolchanges}
            </span>
          )}
          <span className={cn("flex items-center gap-1", afc.error_state ? "text-danger" : working ? "text-warn" : "")}>
            {working && !afc.error_state && <Loader2 size={11} className="animate-spin" />}
            {stateLabel(afc)}
          </span>
          <span className="flex items-center gap-1" title={hubLoaded ? "Filament in the hub" : "Hub empty"}>
            hub <span className={cn("h-1.5 w-1.5 rounded-full", hubLoaded ? "bg-ok" : "bg-fg-3")} />
          </span>
        </span>
      </div>

      {[...byUnit.entries()].map(([unit, lanes]) => (
        <div key={unit} className="flex flex-col gap-1">
          {byUnit.size > 1 && <div className="text-[10px] text-fg-3">{unit}</div>}
          <div className="grid gap-1.5" style={{ gridTemplateColumns: `repeat(${Math.min(4, Math.max(2, lanes.length))}, minmax(0, 1fr))` }}>
            {lanes.map((lane) => (
              <LaneChip
                key={lane.object}
                lane={lane}
                inTool={lane.name === afc.current_load}
                busy={lane.name === afc.current_lane && working}
                armed={armed === lane.name}
                disabled={locked || lane.filament_status === "Not Ready" || lane.name === afc.current_load}
                onClick={() => change(lane)}
              />
            ))}
          </div>
        </div>
      ))}

      <div className="flex items-center gap-2 text-[12px]">
        <span className="min-w-0 flex-1 truncate text-fg-2">
          {loaded ? (
            <>
              Loaded: <span className="font-medium text-fg">{loaded.map || loaded.name}</span>
              {loaded.material ? ` · ${loaded.material}` : ""}
              {loaded.filament_name ? ` · ${loaded.filament_name}` : ""}
            </>
          ) : (
            "No lane in the tool"
          )}
        </span>
        {loaded && (
          <button
            disabled={locked}
            onClick={() => run("gcode", "TOOL_UNLOAD")}
            title="TOOL_UNLOAD"
            className="flex items-center gap-1 rounded-md bg-surface-3 px-2 py-1 text-[12px] font-medium text-fg transition hover:brightness-110 disabled:opacity-50"
          >
            {pending === "gcode:TOOL_UNLOAD" ? <Loader2 size={12} className="animate-spin" /> : <ArrowUpFromLine size={12} />} Unload
          </button>
        )}
      </div>

      {(afc.error_state || afc.message) && (
        <div className={cn("rounded-lg px-2.5 py-1.5 text-[12px]", afc.error_state ? "bg-danger/10 text-danger" : "bg-warn/10 text-warn")}>
          <div className="flex items-start gap-1.5">
            <AlertTriangle size={13} className="mt-0.5 shrink-0" />
            <span className="min-w-0 flex-1 whitespace-pre-wrap">{afc.message || "AFC error"}</span>
          </div>
          {afc.error_state && (
            <button
              disabled={pending != null}
              onClick={() => run("gcode", "AFC_RESUME")}
              className="mt-1.5 flex items-center gap-1 rounded-md bg-danger/15 px-2 py-1 text-[12px] font-medium hover:bg-danger/25 disabled:opacity-50"
            >
              <RotateCcw size={12} /> AFC_RESUME
            </button>
          )}
        </div>
      )}
    </Card>
  );
}

function stateLabel(afc: AfcState) {
  if (afc.error_state) return "error";
  switch (afc.current_state) {
    case "Idle":
    case "Initialized":
      return "ready";
    case "Loading":
      return `loading ${afc.current_lane ?? ""}`.trim();
    case "Unloading":
      return `unloading ${afc.current_lane ?? ""}`.trim();
    case "ToolSwap":
      return "tool change";
    case "Ejecting":
      return "ejecting";
    case "Moving":
      return "moving";
    case "Restoring":
      return "restoring";
    default:
      return afc.current_state.toLowerCase();
  }
}

function LaneChip({
  lane,
  inTool,
  busy,
  armed,
  disabled,
  onClick,
}: {
  lane: AfcLane;
  inTool: boolean;
  busy: boolean;
  armed: boolean;
  disabled: boolean;
  onClick: () => void;
}) {
  const empty = lane.filament_status === "Not Ready";
  const prep = lane.filament_status === "Prep";
  const color = lane.color && lane.color !== "#000000" ? lane.color : null;
  const tip = [
    lane.name,
    lane.filament_name || lane.material || "empty",
    lane.weight > 0 ? `${Math.round(lane.weight)} g` : null,
    lane.filament_status,
    lane.spool_id != null ? `Spoolman #${lane.spool_id}` : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <button
      title={armed ? `Load ${lane.map || lane.name}?` : tip}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "flex flex-col items-start gap-0.5 rounded-lg border px-2 py-1.5 text-left transition",
        inTool
          ? "border-accent bg-accent/10"
          : armed
            ? "border-warn bg-warn/10"
            : "border-line bg-surface-3/60 enabled:hover:bg-surface-3",
        empty && "opacity-50",
        "disabled:cursor-default",
      )}
    >
      <span className="flex w-full items-center gap-1.5">
        <span
          className={cn("h-3 w-3 shrink-0 rounded-full border border-black/20", !color && "bg-fg/15")}
          style={color ? { background: color } : undefined}
        />
        <span className="text-[12px] font-semibold tabular-nums">{lane.map || lane.name}</span>
        {busy ? (
          <Loader2 size={11} className="ml-auto animate-spin text-warn" />
        ) : inTool ? (
          <ArrowDownToLine size={11} className="ml-auto text-accent" />
        ) : prep ? (
          <span className="ml-auto h-1.5 w-1.5 rounded-full bg-warn" title="At the prep sensor, not loaded" />
        ) : null}
      </span>
      <span className="w-full truncate text-[11px] text-fg-2">{armed ? "Load?" : empty ? "empty" : lane.material || "—"}</span>
      {!empty && lane.weight > 0 && <span className="text-[10px] tabular-nums text-fg-3">{Math.round(lane.weight)} g</span>}
    </button>
  );
}
