import { Badge } from "../components/ui/badge";
import { BADGE_LABELS, type HelpBadgeKind } from "./status";

/** Per-row registration status pill (legacy parity): Ready (success tint),
 *  Fallback (accent tint), In use (warning tint). */
export function HotkeyStatusBadge({ kind }: { kind: HelpBadgeKind }) {
  const variant =
    kind === "in-use"
      ? "destructive"
      : kind === "fallback"
        ? "default"
        : kind === "disabled"
          ? "outline"
          : "success";
  return (
    <Badge variant={variant} data-testid="hotkey-status-badge">
      {BADGE_LABELS[kind]}
    </Badge>
  );
}
