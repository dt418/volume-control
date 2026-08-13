import { Badge } from "../components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Kbd } from "../components/ui/kbd";
import { modifierById, type ModifierOption } from "./modifierOptions";

/**
 * Per-action registration outcome mirrored from bootstrap.hotkey_status /
 * state://hotkeys events. serde serializes the unit variant as the string
 * "Registered" and the newtype variant as { "Conflicted": { error_code,
 * message } }.
 */
export type HotkeyRegStatus =
  | "Registered"
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
  modifier: ModifierOption;
  hotkeyStatus: HotkeyRegResult[];
}

/** Card of read-only key combos for the selected modifier, with conflict
 *  badges per action (spec §5.2). */
export function KeyCard({ modifier, hotkeyStatus }: KeyCardProps) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Hotkeys</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {modifier.combos.map((combo) => {
          const conflict = conflictForAction(hotkeyStatus, combo.action);
          return (
            <div
              key={combo.action}
              className="flex items-center justify-between gap-2 text-sm"
            >
              <span className="text-foreground/80">{combo.label}</span>
              <span className="flex items-center gap-2">
                {conflict ? (
                  <Badge variant="destructive" className="destructive">
                    {conflict}
                  </Badge>
                ) : null}
                <Kbd>{combo.combo}</Kbd>
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
