import type { HotkeyRegResult, HotkeyRegStatus } from "../settings/KeyCard";

/** Legacy status pill kinds (help.rs BadgeKind parity): Ready / Fallback /
 *  In use, mapped from the ACTUAL registration outcome. */
export type HelpBadgeKind = "ready" | "fallback" | "in-use";

/** Missing status reads optimistically as Ready (legacy badge_for_action
 *  fallback: "status not yet reported"). */
export function badgeKindFor(status: HotkeyRegStatus | undefined): HelpBadgeKind {
  if (!status) return "ready";
  if (typeof status === "object" && "Conflicted" in status) return "in-use";
  if (status === "HookRouted") return "fallback";
  return "ready";
}

/** The status pill for one base action, from the actual registration status
 *  array (bootstrap.hotkey_status / state://hotkeys). */
export function statusForAction(
  results: HotkeyRegResult[],
  action: string,
): HelpBadgeKind {
  const result = results.find((r) => r.action === action);
  return badgeKindFor(result?.status);
}

export const BADGE_LABELS: Record<HelpBadgeKind, string> = {
  ready: "Ready",
  fallback: "Fallback",
  "in-use": "In use",
};
