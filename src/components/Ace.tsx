import { useEffect, useState } from "react";
import { AlertTriangle, ArrowUpFromLine, Boxes, Droplets, Loader2, Square } from "lucide-react";
import {
  ACE_API_VERSION,
  aceHeadBound,
  activeHeadIndex,
  isBusy,
  type AceHead,
  type AceSlot,
  type AceState,
  type AceUnit,
  type PrinterConfig,
  type Snapshot,
} from "@/lib/types";
import { useAction } from "@/lib/useAction";
import { cn, fmtTemp } from "@/lib/format";
import { Card } from "./ui";

/**
 * MultiACE: the units' slots, what each toolhead is fed from, and the dryers.
 *
 * The slots are the controls — click one to put it in the head it feeds, swap that
 * head over to it, or take it back out — and a loaded toolhead unloads from its own
 * tile too, which is the only way out when its slot is not on screen (its unit
 * offline, or a mapping left stale by an unload done at the printer).
 * Every command names its unit and slot,
 * so the engine's `active_device` (which it moves by itself on a tool change)
 * never has to be explained or operated here.
 *
 * Commands are the stable vocabulary of `multiace/docs/ENGINE_API.md`, not the
 * `ACEx__*` convenience macros, which belong to multiACE's own web UI.
 */
export function AceCard({ printer, snapshot: s, ace }: { printer: PrinterConfig; snapshot: Snapshot; ace: AceState }) {
  const { pending, run } = useAction(printer.id);
  const [armed, setArmed] = useState<string | null>(null);
  useEffect(() => {
    if (!armed) return;
    const t = setTimeout(() => setArmed(null), 3000);
    return () => clearTimeout(t);
  }, [armed]);

  const ready = s.klippy_state === "ready";
  const working = ace.swap_in_progress || ace.unload_all_active || ace.status === "busy";
  const locked = !ready || isBusy(s) || working || pending != null;
  const offline = ace.units.filter((u) => !u.connected);
  // `ace.ace_head` is an echo of the `[ace] ace_head` config line, not a runtime
  // pointer, so the printing marker follows Klipper's own toolhead.
  // `active_extruder` defaults to `extruder` even when idle, so it only marks a
  // head while the printer is actually laying plastic.
  const printingHead = isBusy(s) ? activeHeadIndex(s.active_extruder) : null;
  const newerEngine = ace.api_version > ACE_API_VERSION;

  /** Two-step: first click arms the button, second runs the G-code. */
  const arm = (gcode: string) => {
    if (armed === gcode) {
      setArmed(null);
      run("gcode", gcode);
    } else {
      setArmed(gcode);
    }
  };

  // Units are wired in parallel: in multi mode slot N feeds head N on every unit,
  // so one column count across all grids puts a head above its candidate slots.
  const columns = Math.min(
    4,
    Math.max(2, ace.heads.length, ...ace.units.map((u) => ace.slots.filter((x) => x.ace === u.index).length)),
  );

  /** The head this slot is wired to, loaded or not. */
  const headFor = (slot: AceSlot): AceHead | null =>
    ace.mode === "head"
      ? (ace.heads.find((h) => h.ace === slot.ace) ?? null)
      : (ace.heads.find((h) => h.index === slot.slot) ?? null);

  /** The head this slot is feeding right now, if any. */
  const loadedIn = (slot: AceSlot): AceHead | null =>
    ace.heads.find((h) => aceHeadBound(h) && h.ace === slot.ace && h.slot === slot.slot) ?? null;

  const units: AceUnit[] = ace.units.length
    ? ace.units
    : [
        { index: ace.active_device, connected: true, temp: ace.temp, humidity: null, dryer: ace.dryer },
      ];

  return (
    <Card className="flex flex-col gap-2 p-2.5">
      <div className="flex items-center justify-between">
        <span className="flex items-center gap-1 text-[11px] font-medium uppercase tracking-wide text-fg-2">
          <Boxes size={12} /> MultiACE
          {ace.mode !== "" && ace.mode !== "multi" && (
            <span className="ml-1 rounded-full bg-warn/15 px-1.5 text-[10px] normal-case tracking-normal text-warn">{ace.mode}</span>
          )}
        </span>
        <span className={cn("flex items-center gap-1 text-[11px] text-fg-3", offline.length ? "text-danger" : working ? "text-warn" : "")}>
          {working && <Loader2 size={11} className="animate-spin" />}
          {stateLabel(ace, offline.length > 0)}
        </span>
      </div>

      <div className="text-[10px] text-fg-3">Toolheads</div>
      <div className="-mt-1 grid gap-1.5" style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}>
        {ace.heads.map((head) => {
          const gcode = aceHeadBound(head) ? `ACE_UNLOAD_HEAD HEAD=${head.index}` : null;
          return (
            <HeadTile
              key={head.index}
              head={head}
              printing={head.index === printingHead}
              gcode={gcode}
              armed={gcode != null && armed === gcode}
              disabled={locked || gcode == null}
              busy={pending === `gcode:${gcode}`}
              onClick={() => gcode && arm(gcode)}
            />
          );
        })}
      </div>

      {units.map((unit) => {
        const slots = ace.slots.filter((x) => x.ace === unit.index);
        if (!slots.length) return null;
        const drying = unit.dryer.status !== "" && unit.dryer.status !== "stop";
        const dry = drying ? `ACE_STOP_DRYING ACE=${unit.index}` : `ACE_DRY ACE=${unit.index}`;
        return (
          <div key={unit.index} className="flex flex-col gap-1">
            <div className="flex items-center gap-1.5 text-[10px] text-fg-3">
              <span>ACE {unit.index}</span>
              {!unit.connected && <span className="rounded-full bg-danger/15 px-1.5 text-danger">offline</span>}
              {unit.temp > 0 && <span className="tabular-nums">{fmtTemp(unit.temp)}</span>}
              {unit.humidity != null && (
                <span className="tabular-nums" title="Relative humidity">
                  {Math.round(unit.humidity)}% RH
                </span>
              )}
              {drying && (
                <span className="flex items-center gap-0.5 text-accent" title={`Drying at ${fmtTemp(unit.dryer.target_temp)}`}>
                  <Droplets size={10} />
                  {unit.dryer.remain_time > 0 ? `${unit.dryer.remain_time} min` : unit.dryer.status}
                </span>
              )}
              {/* Each unit dries on its own, so the control belongs on its row. */}
              <button
                disabled={!ready || working || !unit.connected || pending != null}
                onClick={() => run("gcode", dry)}
                title={dry}
                className="ml-auto flex items-center gap-0.5 rounded-full bg-surface-3 px-1.5 text-fg-2 transition hover:brightness-110 disabled:opacity-50"
              >
                {pending === `gcode:${dry}` ? (
                  <Loader2 size={10} className="animate-spin" />
                ) : drying ? (
                  <Square size={10} />
                ) : (
                  <Droplets size={10} />
                )}
                {drying ? "stop" : "dry"}
              </button>
            </div>
            <div className="grid gap-1.5" style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}>
              {slots.map((slot) => {
                const loaded = loadedIn(slot);
                const target = headFor(slot);
                const gcode = slotCommand(slot, loaded, target);
                return (
                  <SlotTile
                    key={`${slot.ace}_${slot.slot}`}
                    slot={slot}
                    loaded={loaded}
                    target={target}
                    // Multi mode wires slot N to head N, so the column says it. Head mode
                    // lets a head take any slot of its unit, and then the tile must.
                    showHead={ace.mode === "head"}
                    gcode={gcode}
                    armed={gcode != null && armed === gcode}
                    disabled={locked || !unit.connected || gcode == null}
                    busy={pending === `gcode:${gcode}`}
                    onClick={() => gcode && arm(gcode)}
                  />
                );
              })}
            </div>
          </div>
        );
      })}

      <div className="flex items-center gap-2 text-[12px]">
        <span className="min-w-0 flex-1 truncate text-fg-2">
          {ace.device_count > 1 ? `${ace.device_count} units` : "1 unit"}
          {ace.spoolman ? " · Spoolman" : ""}
        </span>
        {ace.heads.some(aceHeadBound) && (
          <button
            disabled={locked}
            onClick={() => arm("ACE_UNLOAD_ALL_HEADS")}
            title="ACE_UNLOAD_ALL_HEADS"
            className={cn(
              "flex items-center gap-1 rounded-md px-2 py-1 text-[12px] font-medium transition hover:brightness-110 disabled:opacity-50",
              armed === "ACE_UNLOAD_ALL_HEADS" ? "bg-warn/20 text-warn" : "bg-surface-3 text-fg",
            )}
          >
            {pending === "gcode:ACE_UNLOAD_ALL_HEADS" ? <Loader2 size={12} className="animate-spin" /> : <ArrowUpFromLine size={12} />}
            {armed === "ACE_UNLOAD_ALL_HEADS" ? "Sure?" : "Unload all heads"}
          </button>
        )}
      </div>

      {(newerEngine || offline.length > 0 || ace.last_swap_result || ace.calibration_state === "running") && (
        <div
          className={cn(
            "rounded-lg px-2.5 py-1.5 text-[12px]",
            offline.length || ace.last_swap_result ? "bg-danger/10 text-danger" : "bg-warn/10 text-warn",
          )}
        >
          <div className="flex items-start gap-1.5">
            <AlertTriangle size={13} className="mt-0.5 shrink-0" />
            <span className="min-w-0 flex-1 whitespace-pre-wrap">
              {offline.length
                ? `ACE ${offline.map((u) => u.index).join(", ")} not answering`
                : ace.last_swap_result
                  ? `Last swap: ${ace.last_swap_result}`
                  : ace.calibration_state === "running"
                    ? "Bowden calibration in progress"
                    : `multiACE engine API ${ace.api_version}; this card knows ${ACE_API_VERSION}`}
            </span>
          </div>
        </div>
      )}
    </Card>
  );
}

/**
 * What clicking a slot does: take it out of its head, put it in, or swap the
 * head over from the slot it currently holds. `null` = nothing sensible to do.
 */
function slotCommand(slot: AceSlot, loaded: AceHead | null, target: AceHead | null): string | null {
  if (loaded) return `ACE_UNLOAD_HEAD HEAD=${loaded.index}`;
  if (!target || target.manual || target.feeder || !slot.occupied) return null;
  return aceHeadBound(target)
    ? `ACE_SWAP_HEAD HEAD=${target.index} ACE=${slot.ace} SLOT=${slot.slot}`
    : `ACE_LOAD_HEAD HEAD=${target.index} ACE=${slot.ace} SLOT=${slot.slot}`;
}

function stateLabel(ace: AceState, offline: boolean) {
  if (offline) return "offline";
  if (ace.unload_all_active) return "unloading all";
  if (ace.swap_in_progress || (ace.swap_phase !== "" && ace.swap_phase !== "idle")) {
    return ace.swap_phase && ace.swap_phase !== "idle" ? `swap · ${ace.swap_phase}` : "swapping";
  }
  if (ace.status === "ready") return "ready";
  // `unknown` is the engine's "no reading", not a fault.
  return ace.status.toLowerCase() || "—";
}

/**
 * Border and fill shared by both rows: lit when loaded (accent for a slot, the
 * hot-end colour for a head), warn while armed, dashed when the ACE does not
 * feed it, dim when there is nothing there or nothing to do.
 */
function tileClasses(o: { lit?: "accent" | "heat"; armed?: boolean; dimmed?: boolean; dashed?: boolean; hover?: boolean }) {
  return cn(
    "rounded-lg border border-line bg-surface-3/40 px-2 text-left transition disabled:cursor-default",
    o.lit === "accent" && "border-accent bg-accent/10",
    o.lit === "heat" && "border-heat bg-heat/10",
    o.dashed && "border-dashed",
    o.armed && "border-warn bg-warn/10",
    o.dimmed && "opacity-50",
    o.hover && "enabled:hover:brightness-125",
  );
}

function Swatch({ color }: { color: string }) {
  const c = color && color !== "#000000" ? color : null;
  return (
    <span
      className={cn("h-3 w-3 shrink-0 rounded-full border border-black/20", !c && "bg-fg/15")}
      style={c ? { background: c } : undefined}
    />
  );
}

/**
 * A slim strip over the slot grid. It carries only what the slots cannot: which
 * head is laying plastic, which is hand- or feeder-fed, which is empty, and the
 * way out of a head whose slot the grid below cannot show. What is *in* the head
 * is the accent slot in the same column, so it is not repeated here.
 */
function HeadTile({
  head,
  printing,
  gcode,
  armed,
  disabled,
  busy,
  onClick,
}: {
  head: AceHead;
  printing: boolean;
  gcode: string | null;
  armed: boolean;
  disabled: boolean;
  busy: boolean;
  onClick: () => void;
}) {
  const bound = aceHeadBound(head);
  const mode = head.manual ? "manual" : head.feeder ? "feeder" : null;
  const tip = [
    `T${head.index}`,
    printing ? "printing now" : null,
    mode ?? (bound ? `from ACE ${head.ace} slot ${head.slot}` : "nothing loaded"),
    [head.vendor, head.material, head.subtype].filter(Boolean).join(" ") || null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <button
      title={armed && gcode ? `${gcode}?` : tip}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "flex items-center gap-1.5 py-1",
        // Dashed = hand- or feeder-fed: the ACE never touches it, and it owns no slot below.
        tileClasses({ lit: bound ? "heat" : undefined, armed, dimmed: !bound && !mode, dashed: !!mode, hover: !!gcode }),
      )}
    >
      <Swatch color={head.color} />
      <span className="text-[12px] font-semibold tabular-nums">T{head.index}</span>
      {armed && <span className="truncate text-[11px] text-warn">out?</span>}
      {busy ? (
        <Loader2 size={11} className="ml-auto animate-spin text-warn" />
      ) : printing ? (
        <span className="pulse-soft ml-auto h-1.5 w-1.5 rounded-full bg-ok" title="Printing now" />
      ) : null}
    </button>
  );
}

/** The control: click to load this slot into its head, or take it back out. */
function SlotTile({
  slot,
  loaded,
  target,
  showHead,
  gcode,
  armed,
  disabled,
  busy,
  onClick,
}: {
  slot: AceSlot;
  loaded: AceHead | null;
  /** The head this slot is wired to; named in the tooltip, and on the tile only
   * when the column cannot say it (head mode). */
  target: AceHead | null;
  showHead: boolean;
  gcode: string | null;
  armed: boolean;
  disabled: boolean;
  busy: boolean;
  onClick: () => void;
}) {
  const verb = !gcode ? null : loaded ? "Unload?" : gcode.startsWith("ACE_SWAP_HEAD") ? "Swap?" : "Load?";
  const tip = [
    `Slot ${slot.slot}`,
    slot.label || [slot.vendor, slot.material].filter(Boolean).join(" ") || "empty",
    slot.weight > 0 ? `${Math.round(slot.weight)} g` : null,
    loaded ? `loaded in T${loaded.index}` : target && slot.occupied ? `feeds T${target.index}` : null,
    slot.occupied ? null : "gate empty",
    slot.spool_id != null ? `Spoolman #${slot.spool_id}` : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <button
      title={armed && gcode ? `${gcode}?` : tip}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "flex flex-col items-start gap-0.5 py-1.5",
        tileClasses({
          lit: loaded ? "accent" : undefined,
          armed,
          dimmed: !slot.occupied && !slot.material,
          hover: !!gcode,
        }),
      )}
    >
      <span className="flex w-full items-center gap-1.5">
        <Swatch color={slot.color} />
        <span className="text-[12px] font-semibold tabular-nums">S{slot.slot}</span>
        {busy ? (
          <Loader2 size={11} className="ml-auto animate-spin text-warn" />
        ) : showHead && (loaded ?? target) ? (
          <span className={cn("ml-auto text-[10px] tabular-nums", loaded ? "text-accent" : "text-fg-3")}>
            T{(loaded ?? target)!.index}
          </span>
        ) : !slot.occupied && slot.material ? (
          <span className="ml-auto h-1.5 w-1.5 rounded-full bg-fg-3" title="Gate empty" />
        ) : null}
      </span>
      <span className="w-full truncate text-[11px] text-fg-2">{armed && verb ? verb : slot.material || "empty"}</span>
      {slot.weight > 0 && <span className="text-[10px] tabular-nums text-fg-3">{Math.round(slot.weight)} g</span>}
    </button>
  );
}
