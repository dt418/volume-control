import { Badge } from "../components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Kbd } from "../components/ui/kbd";
import { HOTKEY_ACTIONS, formatShortcut, modifierById, type ModifierOption } from "./modifierOptions";
import type { HotkeyBindings } from "./settingsTypes";

/**
 * Per-action registration outcome mirrored from bootstrap.hotkey_status /
 * state://hotkeys events. serde serializes the unit variant as the string
 * "Registered" and the newtype variant as { "Conflicted": { error_code,
 * message } }.
 */
export type HotkeyRegStatus =
  | "Registered"
  | "Disabled"
  | "HookRouted"
  | { Conflicted: { error_code: number; message: string } };

export interface HotkeyRegResult {
  action: string;
  status: HotkeyRegStatus;
}

export function conflictMessage(result: HotkeyRegResult): string | null {
  if (typeof result.status === "object" && "Conflicted" in result.status) {
    return result.status.Conflicted.message;
  }
  return null;
}

export function conflictForAction(
  results: HotkeyRegResult[],
  action: string,
): string | null {
  for (const result of results) {
    if (result.action === action) return conflictMessage(result);
  }
  return null;
}

export interface KeyCardProps {
  bindings: HotkeyBindings;
  hotkeyStatus: HotkeyRegResult[];
}

/** Card of read-only key combos for the selected modifier, with conflict
 *  badges per action (spec §5.2). */
export function KeyCard({ bindings, hotkeyStatus }: KeyCardProps) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Hotkeys</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {HOTKEY_ACTIONS.map((entry) => {
          const conflict = conflictForAction(hotkeyStatus, entry.action);
          const shortcut = bindings[entry.key];
          return (
            <div
              key={entry.action}
              className="flex items-center justify-between gap-2 text-sm"
            >
              <span className="text-foreground/80">{entry.label}</span>
              <span className="flex items-center gap-2">
                {conflict ? (
                  <Badge variant="destructive" className="destructive">
                    {conflict}
                  </Badge>
                ) : null}
                <Kbd>{formatShortcut(shortcut)}</Kbd>
              </span>
            </div>
          );
        })}
      </CardContent>
    </Card>
  );
}

/** Selected modifier resolved from the config value (falls back to CtrlAlt). */
export function modifierForId(id: string | undefined): ModifierOption {
  return modifierById(id ?? "CtrlAlt");
}
