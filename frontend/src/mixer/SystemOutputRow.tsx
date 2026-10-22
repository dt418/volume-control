import { useEffect, useRef, useState } from "react";
import { RotateCcw, Volume2, VolumeX } from "lucide-react";

import { Slider } from "../components/ui/slider";
import { useT } from "../lib/i18n";
import { invoke } from "../lib/ipc";
import { bandForValue, type ColorThresholds } from "./SignalRail";

interface SystemOutputRowProps {
  value: number;
  muted: boolean;
  thresholds: ColorThresholds;
  onError?: (error: unknown) => void;
}

/**
 * System-output slider with the §9.1 echo-jitter guard (same pattern as
 * AppSlider, but against the host `set_volume` command): optimistic local
 * state + an `isDragging` ref so `state://volume` refreshes never fight the
 * thumb while the user is dragging.
 */
function SystemVolumeSlider({
  value,
  muted,
  thresholds,
}: Pick<SystemOutputRowProps, "value" | "muted" | "thresholds">) {
  const [localVal, setLocalVal] = useState(value);
  const isDragging = useRef(false);

  useEffect(() => {
    if (!isDragging.current) setLocalVal(value);
  }, [value]);

  const handleChange = (newVal: number) => {
    setLocalVal(newVal); // immediate UI
    void invoke<void>("set_volume", { percent: newVal }).catch(() => {});
  };

  const band = bandForValue(localVal, muted, thresholds);

  return (
    <Slider
      value={[localVal]}
      min={0}
      max={100}
      step={1}
      aria-label="System output volume"
      rangeClassName={`rail-fill band-${band}`}
      onPointerDown={() => (isDragging.current = true)}
      onPointerUp={() => (isDragging.current = false)}
      onValueChange={([val]) => handleChange(val)}
    />
  );
}

/**
 * Global system-output header card: live value/Muted from `state://volume` +
 * `get_bootstrap.volume_pct/muted`, a threshold-colored slider (`set_volume`),
 * Mute/Unmute (`toggle_mute`) and `Reset volume to 50%` (`reset_volume`).
 *
 * The compact single-row actions keep the fixed 400x224 mixer useful: the
 * session list still gets vertical room below the system card.
 */
export function SystemOutputRow({ value, muted, thresholds, onError }: SystemOutputRowProps) {
  const MuteIcon = muted ? VolumeX : Volume2;
  const tr = useT();
  return (
    <section
      data-testid="system-output-row"
      aria-label={tr("common.systemOutput")}
      className="rounded-lg border border-foreground/20 bg-background/88 px-2.5 py-1.5 shadow-sm"
    >
      <div className="flex items-center gap-2">
        <h2 className="text-xs font-semibold">{tr("common.systemOutput")}</h2>
        <span data-testid="system-output-value" className="text-xs font-medium">
          {muted ? tr("common.muted") : `${value}%`}
        </span>
        <div className="ml-auto flex items-center gap-1">
          <button
            type="button"
            aria-label={muted ? tr("mixer.unmuteSystemOutput") : tr("mixer.muteSystemOutput")}
            aria-pressed={muted}
            title={muted ? tr("mixer.unmuteSystemOutput") : tr("mixer.muteSystemOutput")}
            onClick={() => void invoke<void>("toggle_mute").catch((error) => onError?.(error))}
            className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-foreground/20 bg-foreground/10 text-foreground/90 transition-colors hover:bg-foreground/15 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            <MuteIcon aria-hidden="true" className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            aria-label={tr("mixer.resetVolume")}
            title={tr("mixer.resetVolume")}
            onClick={() => void invoke<void>("reset_volume").catch((error) => onError?.(error))}
            className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-foreground/20 bg-foreground/10 text-foreground/90 transition-colors hover:bg-foreground/15 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            <RotateCcw aria-hidden="true" className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
      <div className="mt-1.5">
        <SystemVolumeSlider value={value} muted={muted} thresholds={thresholds} />
      </div>
    </section>
  );
}
