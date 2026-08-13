import { useEffect, useRef, useState } from "react";
import { Volume2, VolumeX } from "lucide-react";

import { Slider } from "../components/ui/slider";
import { invoke } from "../lib/ipc";
import { SignalRail, type ColorThresholds } from "./SignalRail";

interface SystemOutputRowProps {
  value: number;
  muted: boolean;
  thresholds: ColorThresholds;
}

/**
 * System-output slider with the §9.1 echo-jitter guard (same pattern as
 * AppSlider, but against the host `set_volume` command): optimistic local
 * state + an `isDragging` ref so `state://volume` refreshes never fight the
 * thumb while the user is dragging.
 */
function SystemVolumeSlider({ value }: { value: number }) {
  const [localVal, setLocalVal] = useState(value);
  const isDragging = useRef(false);

  useEffect(() => {
    if (!isDragging.current) setLocalVal(value);
  }, [value]);

  const handleChange = (newVal: number) => {
    setLocalVal(newVal); // immediate UI
    void invoke<void>("set_volume", { percent: newVal }).catch(() => {});
  };

  return (
    <Slider
      value={[localVal]}
      min={0}
      max={100}
      step={1}
      aria-label="System output volume"
      onPointerDown={() => (isDragging.current = true)}
      onPointerUp={() => (isDragging.current = false)}
      onValueChange={([val]) => handleChange(val)}
    />
  );
}

/**
 * Global `System output` header card (legacy mixer parity, gap analysis
 * "Global system volume *" rows): live value/Muted from `state://volume` +
 * `get_bootstrap.volume_pct/muted`, a threshold-aware SignalRail, a system
 * slider (`set_volume`), Mute/Unmute (`toggle_mute`) and `Reset volume to
 * 50%` (`reset_volume`).
 */
export function SystemOutputRow({ value, muted, thresholds }: SystemOutputRowProps) {
  const MuteIcon = muted ? VolumeX : Volume2;
  return (
    <section
      data-testid="system-output-row"
      aria-label="System output"
      className="rounded-lg border border-foreground/10 bg-background/60 px-3 py-2"
    >
      <div className="flex items-baseline justify-between">
        <h2 className="text-xs font-semibold">System output</h2>
        <span data-testid="system-output-value" className="text-xs font-medium">
          {muted ? "Muted" : `${value}%`}
        </span>
      </div>
      <div className="mt-2">
        <SignalRail value={value} muted={muted} thresholds={thresholds} />
      </div>
      <div className="mt-2">
        <SystemVolumeSlider value={value} />
      </div>
      <div className="mt-2 flex items-center gap-2">
        <button
          type="button"
          aria-label={muted ? "Unmute system output" : "Mute system output"}
          onClick={() => void invoke<void>("toggle_mute").catch(() => {})}
          className="inline-flex items-center gap-1 rounded-md border border-foreground/10 bg-foreground/5 px-2 py-1 text-xs text-foreground/80 transition-colors hover:bg-foreground/10 hover:text-foreground"
        >
          <MuteIcon className="h-3.5 w-3.5" />
          {muted ? "Unmute" : "Mute"}
        </button>
        <button
          type="button"
          aria-label="Reset volume to 50%"
          onClick={() => void invoke<void>("reset_volume").catch(() => {})}
          className="rounded-md border border-foreground/10 bg-foreground/5 px-2 py-1 text-xs text-foreground/80 transition-colors hover:bg-foreground/10 hover:text-foreground"
        >
          Reset volume to 50%
        </button>
      </div>
    </section>
  );
}
