import { Badge } from "../components/ui/badge";
import { useT } from "../lib/i18n";
import { BADGE_LABELS, type HelpBadgeKind } from "./status";

/** Per-row registration status pill (legacy parity): Ready (success tint),
 *  Fallback (accent tint), In use (warning tint). Only `In use` is
 *  translatable today — the other labels have no dictionary entry yet. */
export function HotkeyStatusBadge({ kind }: { kind: HelpBadgeKind }) {
  const tr = useT();
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
      {kind === "in-use" ? tr("help.statusInUse") : tr(BADGE_LABELS[kind])}
    </Badge>
  );
}
