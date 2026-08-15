import { SignalRail } from "../mixer/SignalRail";
import type { SettingsConfig } from "./settingsTypes";

/** Draft-driven mini Signal Rail preview (legacy parity: "preview tracks the
 *  draft without mutating host state; only Apply persists"). Renders with the
 *  draft thresholds at a fixed sample value so the user sees band colors as
 *  they edit. */
export function AppearancePreview({ draft }: { draft: SettingsConfig }) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-sm font-semibold">Preview</span>
      <SignalRail value={65} muted={false} thresholds={draft.color_thresholds} />
      <span className="text-xs text-foreground/60">
        preview tracks the draft; only Save applies
      </span>
    </div>
  );
}
