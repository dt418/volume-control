import { cn } from "../lib/utils";

/** The user's `config.color_thresholds` band boundaries (VolumePro palette). */
export interface ColorThresholds {
  green_up_to: number;
  blue_up_to: number;
  orange_up_to: number;
}

/** Config defaults; used until the bootstrap payload arrives. */
export const DEFAULT_THRESHOLDS: ColorThresholds = {
  green_up_to: 40,
  blue_up_to: 75,
  orange_up_to: 100,
};

export type RailBand = "gray" | "green" | "blue" | "orange";

/**
 * Band for a value, mirroring the authoritative `core::volume_color_rgb`
 * semantics (muted or 0% -> gray, then the green/blue/orange threshold
 * bands from the user's config).
 */
export function bandForValue(
  value: number,
  muted: boolean,
  thresholds: ColorThresholds,
): RailBand {
  if (muted || value <= 0) return "gray";
  if (value <= thresholds.green_up_to) return "green";
  if (value <= thresholds.blue_up_to) return "blue";
  return "orange";
}

interface SignalRailProps {
  value: number;
  muted: boolean;
  thresholds: ColorThresholds;
}

/**
 * Threshold-aware Signal Rail (legacy parity): a track whose fill is colored
 * by the value's band, a circle thumb normally and an outline diamond plus a
 * `Muted` label when muted — the shape carries the muted state so it never
 * relies on color alone (spec §6.3).
 */
export function SignalRail({ value, muted, thresholds }: SignalRailProps) {
  const clamped = Math.max(0, Math.min(100, value));
  const band = bandForValue(clamped, muted, thresholds);
  return (
    <div data-testid="signal-rail" className="relative h-4 w-full">
      <div
        aria-hidden="true"
        className="absolute inset-x-0 top-1/2 h-1.5 -translate-y-1/2 rounded-full bg-foreground/20"
      />
      <div
        aria-hidden="true"
        className={cn(
          "rail-fill absolute left-0 top-1/2 h-1.5 -translate-y-1/2 rounded-full",
          `band-${band}`,
        )}
        style={{ width: `${clamped}%` }}
      />
      {muted ? (
        <>
          <span
            aria-hidden="true"
            className="rail-muted-diamond absolute top-1/2 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rotate-45 border border-foreground bg-background/88"
            style={{ left: `${clamped}%` }}
          />
          <span
            className="absolute -top-0.5 -translate-x-1/2 text-[9px] font-medium uppercase tracking-wide text-foreground/80"
            style={{ left: `${clamped}%` }}
          >
            Muted
          </span>
        </>
      ) : (
        <span
          aria-hidden="true"
          className="rail-thumb absolute top-1/2 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full border border-foreground/40 bg-background"
          style={{ left: `${clamped}%` }}
        />
      )}
    </div>
  );
}
