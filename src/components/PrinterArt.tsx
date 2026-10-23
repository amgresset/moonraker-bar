import { useId } from "react";
import type { Model } from "@/lib/model";

export type ArtState = "printing" | "paused" | "complete" | "error" | "idle" | "offline";

interface Props {
  model: Model;
  /** 0..1, height of the part on the bed. */
  progress: number;
  state: ArtState;
  /** Filament color of the part. */
  color?: string | null;
  size?: number;
  className?: string;
  /** Toolchangers: filament color per docked tool, and the tool on the carriage. */
  tools?: { colors: (string | null)[]; active: number };
}

type Box = { x: number; y: number; w: number; h: number };

const FRAME: Record<Model, Box> = {
  voron24: { x: 12, y: 18, w: 96, h: 94 },
  trident: { x: 12, y: 14, w: 96, h: 98 },
  generic: { x: 12, y: 14, w: 96, h: 98 },
  v0: { x: 26, y: 32, w: 68, h: 74 },
  snapmaker_u1: { x: 12, y: 14, w: 96, h: 98 },
  bedslinger: { x: 18, y: 16, w: 84, h: 92 },
};

/** CoreXY with a fixed gantry where the bed drops as the part grows. */
const MOVING_BED: Record<Model, boolean> = { voron24: false, generic: false, bedslinger: false, trident: true, v0: true, snapmaker_u1: true };

const hex = (c?: string | null) =>
  c && /^#?[0-9a-f]{6}$/i.test(c) ? (c.startsWith("#") ? c : `#${c}`) : null;

/**
 * A line drawing of the printer, with the part growing on the bed in the filament
 * color and the toolhead sweeping while it prints. Matches each machine's motion:
 * a 2.4 raises its gantry, a Trident or V0 lowers its bed.
 */
export function PrinterArt({ model, progress, state, color, size = 104, className, tools }: Props) {
  const uid = useId().replace(/:/g, "");
  const f = FRAME[model];
  const small = model === "v0";
  const u1 = model === "snapmaker_u1";
  const ix0 = f.x + (small ? 7 : 9);
  const ix1 = f.x + f.w - (small ? 7 : 9);
  const top = f.y + (small ? 6 : u1 ? 22 : 8);
  const bottom = f.y + f.h - (small ? 6 : 8);

  const thW = small ? 11 : 15;
  const thH = small ? 9 : 12;
  const nozzle = small ? 3 : 4;
  const bedH = small ? 3 : 4;
  const bedW = ix1 - ix0 - (small ? 4 : 8);
  const bedX = (ix0 + ix1) / 2 - bedW / 2;
  const partW = Math.round(bedW * 0.42);
  const partX = (ix0 + ix1) / 2 - partW / 2;
  const partMax = bottom - top - thH - nozzle - bedH - 6;

  const p = state === "complete" ? 1 : state === "idle" || state === "offline" ? 0 : Math.min(1, Math.max(0, progress));
  const partH = p * partMax;
  const parked = state === "idle" || state === "offline" || state === "complete";

  let bedY: number, thTop: number;
  if (MOVING_BED[model]) {
    thTop = top + 2;
    bedY = thTop + thH + nozzle + partH;
  } else {
    bedY = bottom - bedH;
    thTop = parked ? top + 2 : bedY - partH - nozzle - thH;
  }
  const partTop = bedY - partH;
  const gantryY = thTop + thH / 2 - 2;

  const frameStroke = state === "error" ? "var(--color-danger)" : "var(--color-fg-3)";
  const led =
    state === "printing" ? "var(--color-accent)" : state === "paused" ? "var(--color-warn)" : state === "error" ? "var(--color-danger)" : "var(--color-fg-3)";
  const active = tools?.active ?? 0;
  const fill = hex(color) ?? hex(tools?.colors[active]) ?? "var(--color-accent)";
  const sweep = Math.max(0, partW / 2 - 2);

  return (
    <svg
      viewBox="0 0 120 120"
      width={size}
      height={size}
      className={className}
      style={{ opacity: state === "offline" ? 0.45 : 1 }}
      aria-hidden="true"
    >
      <defs>
        <pattern id={`layers-${uid}`} width="4" height={small ? 2 : 2.5} patternUnits="userSpaceOnUse">
          <rect width="4" height="0.6" fill="rgba(0,0,0,0.18)" />
        </pattern>
      </defs>

      {/* frame */}
      {model === "bedslinger" ? (
        <g stroke={frameStroke} strokeWidth="3" strokeLinecap="round" fill="none">
          <line x1={f.x + 4} y1={f.y} x2={f.x + 4} y2={f.y + f.h - 6} />
          <line x1={f.x + f.w - 4} y1={f.y} x2={f.x + f.w - 4} y2={f.y + f.h - 6} />
          <line x1={f.x + 4} y1={f.y} x2={f.x + f.w - 4} y2={f.y} />
          <line x1={f.x - 6} y1={f.y + f.h} x2={f.x + f.w + 6} y2={f.y + f.h} />
        </g>
      ) : (
        <g>
          {model === "voron24" && (
            <path
              d={`M${f.x + 8} ${f.y} L${f.x + 14} ${f.y - 9} H${f.x + f.w - 14} L${f.x + f.w - 8} ${f.y} Z`}
              fill="var(--color-fg)"
              fillOpacity="0.04"
              stroke={frameStroke}
              strokeWidth="2"
              strokeLinejoin="round"
            />
          )}
          {model !== "generic" && (
            <rect x={f.x + 3} y={f.y + f.h} width={f.w - 6} height={small ? 4 : 6} rx="2" fill="var(--color-fg)" fillOpacity="0.12" />
          )}
          <rect x={f.x} y={f.y} width={f.w} height={f.h} rx="4" fill="var(--color-fg)" fillOpacity="0.05" stroke={frameStroke} strokeWidth="3" />
          {/* glass glare */}
          {!u1 && <line
            x1={f.x + f.w * 0.72}
            y1={f.y + 5}
            x2={f.x + f.w * 0.52}
            y2={f.y + f.h - 5}
            stroke="white"
            strokeOpacity="0.05"
            strokeWidth={small ? 6 : 9}
            strokeLinecap="round"
          />}
          {/* corner extrusions */}
          <g stroke={frameStroke} strokeWidth="1" opacity="0.5">
            <line x1={f.x + 4} y1={f.y + 3} x2={f.x + 4} y2={f.y + f.h - 3} />
            <line x1={f.x + f.w - 4} y1={f.y + 3} x2={f.x + f.w - 4} y2={f.y + f.h - 3} />
          </g>
        </g>
      )}

      {u1 && (
        <g>
          {/* rear tool docks */}
          <line x1={f.x + 6} y1={f.y + 19} x2={f.x + f.w - 6} y2={f.y + 19} stroke="var(--color-fg-3)" strokeWidth="1.5" opacity="0.6" />
          {[0, 1, 2, 3].map((i) => {
            const dw = 15;
            const gap = (f.w - 12 - dw * 4) / 3;
            const dx = f.x + 6 + i * (dw + gap);
            const docked = parked || i !== active;
            const c = hex(tools?.colors[i]);
            return (
              <g key={i}>
                <rect x={dx} y={f.y + 17} width={dw} height="3" rx="1" fill="var(--color-fg-3)" opacity="0.8" />
                {docked ? (
                  <g>
                    <rect x={dx + 1.5} y={f.y + 6} width={dw - 3} height="10" rx="2" fill="var(--color-surface-3)" stroke="var(--color-fg-2)" strokeWidth="1.2" />
                    <rect x={dx + 4} y={f.y + 8.5} width={dw - 8} height="3.5" rx="1.2" fill={c ?? "var(--color-fg-3)"} fillOpacity={c ? 1 : 0.4} />
                  </g>
                ) : (
                  <rect x={dx + 1.5} y={f.y + 6} width={dw - 3} height="10" rx="2" fill="none" stroke="var(--color-fg-3)" strokeWidth="1" strokeDasharray="2 1.5" opacity="0.7" />
                )}
              </g>
            );
          })}
        </g>
      )}

      {/* bed */}
      {model === "bedslinger" && <line x1={bedX + 4} y1={bedY + bedH + 3} x2={bedX + bedW - 4} y2={bedY + bedH + 3} stroke="var(--color-fg-3)" strokeWidth="2" strokeLinecap="round" />}
      <rect x={bedX} y={bedY} width={bedW} height={bedH} rx="1.5" fill="var(--color-fg-2)" fillOpacity="0.85" />
      {MOVING_BED[model] && (
        <g stroke="var(--color-fg-3)" strokeWidth="1.5" opacity="0.6">
          <line x1={bedX + 6} y1={bedY + bedH} x2={bedX + 6} y2={bottom} />
          <line x1={bedX + bedW - 6} y1={bedY + bedH} x2={bedX + bedW - 6} y2={bottom} />
        </g>
      )}

      {/* part */}
      {partH > 0.5 && (
        <g>
          <rect x={partX} y={partTop} width={partW} height={partH} rx="1.5" fill={fill} stroke="rgba(0,0,0,0.25)" strokeWidth="0.8" />
          <rect x={partX} y={partTop} width={partW} height={partH} rx="1.5" fill={`url(#layers-${uid})`} />
        </g>
      )}

      {/* gantry + toolhead */}
      <rect x={ix0 - 2} y={gantryY} width={ix1 - ix0 + 4} height={small ? 3 : 4} rx="1.5" fill="var(--color-fg-3)" />
      <g>
        {state === "printing" && (
          <animateTransform attributeName="transform" type="translate" values={`${-sweep} 0; ${sweep} 0; ${-sweep} 0`} dur="2.4s" repeatCount="indefinite" />
        )}
        <rect
          x={(ix0 + ix1) / 2 - thW / 2}
          y={thTop}
          width={thW}
          height={thH}
          rx="2.5"
          fill="var(--color-surface-3)"
          stroke="var(--color-fg-2)"
          strokeWidth="1.5"
        />
        {u1 && !parked && (
          <rect x={(ix0 + ix1) / 2 - thW / 2 + 3} y={thTop + 2.5} width={thW - 6} height="3.5" rx="1.2" fill={hex(tools?.colors[active]) ?? "var(--color-fg-3)"} />
        )}
        <rect x={(ix0 + ix1) / 2 - thW / 2 + 3} y={thTop + thH - 3.5} width={thW - 6} height="1.6" rx="0.8" fill={led} />
        <path
          d={`M${(ix0 + ix1) / 2 - 2.5} ${thTop + thH} L${(ix0 + ix1) / 2 + 2.5} ${thTop + thH} L${(ix0 + ix1) / 2} ${thTop + thH + nozzle} Z`}
          fill="var(--color-fg-2)"
        />
      </g>
    </svg>
  );
}
