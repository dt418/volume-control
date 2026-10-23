import { Badge } from "../components/ui/badge";
import { SignalRail } from "../mixer/SignalRail";
import { useT } from "../lib/i18n";
import type { SettingsConfig } from "./settingsTypes";

/** Draft-driven Liquid Glass optical preview card. Renders a simulated
 *  glass surface with draft material, accent, and color thresholds. */
export function AppearancePreview({ draft }: { draft: SettingsConfig }) {
  const tr = useT();
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <span className="text-sm font-semibold">{tr("appearance.previewTitle")}</span>
        <div className="flex items-center gap-1.5">
          <Badge variant="pill" className="text-[10px]">
            {draft.appearance.material}
          </Badge>
          <Badge variant="pill" className="text-[10px]">
            {draft.appearance.accent}
          </Badge>
        </div>
      </div>

      <div
        data-testid="liquid-glass-preview-card"
        className="glass-surface rounded-xl p-3 flex flex-col gap-2 shadow-sm transition-all"
      >
        <div className="flex items-center justify-between text-xs text-foreground/80">
          <span className="font-medium">{tr("appearance.specularRim")}</span>
          <span className="text-[10px] text-foreground/60">{draft.appearance.theme}</span>
        </div>
        <SignalRail value={65} muted={false} thresholds={draft.color_thresholds} />
      </div>

      <span className="text-xs text-foreground/60">
        preview tracks the draft; only Save applies
      </span>
    </div>
  );
}
