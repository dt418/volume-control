import { Badge } from "../components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import type { DraftSetter, SettingsConfig } from "./settingsTypes";
import { conflictForAction, type HotkeyRegResult } from "./KeyCard";
import { ModifierPicker } from "./ModifierPicker";
import { bindingsForModifier, HOTKEY_ACTIONS } from "./modifierOptions";
import { ShortcutRecorder } from "./ShortcutRecorder";
import { FieldError } from "./FieldError";
import type { FieldErrors } from "./settingsTypes";

/** Keyboard section: preset shortcuts plus per-action recorders. */
export function HotkeysSection({
  draft,
  setDraft,
  hotkeyStatus,
  errors,
}: {
  draft: SettingsConfig;
  setDraft: DraftSetter;
  hotkeyStatus: HotkeyRegResult[];
  errors: FieldErrors;
}) {
  const bindings = draft.hotkeys ?? bindingsForModifier(draft.modifier);
  return (
    <Card>
      <CardHeader>
        <CardTitle>Keyboard</CardTitle>
        <p className="text-xs text-foreground/60">
          Press Record, then press a modifier and key. Changes stay in the draft until you save.
        </p>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="flex flex-col gap-2" role="group" aria-label="Global shortcuts">
          {HOTKEY_ACTIONS.map((entry) => {
            const conflict = conflictForAction(hotkeyStatus, entry.action);
            return (
              <div
                key={entry.key}
                className="flex flex-col gap-2 rounded-md border border-foreground/10 p-2 min-[760px]:grid min-[760px]:grid-cols-[minmax(9rem,1fr)_minmax(17rem,auto)] min-[760px]:items-center"
              >
                <div className="min-w-0">
                  <p className="text-sm font-medium">{entry.label}</p>
                  <p className="text-xs text-foreground/60">{entry.help}</p>
                </div>
                <div className="flex min-w-0 flex-wrap items-center justify-end gap-2">
                  <ShortcutRecorder
                    id={entry.key}
                    value={bindings[entry.key]}
                    aria-label={`${entry.label} shortcut`}
                    onChange={(value) =>
                      setDraft((current) => ({
                        ...current,
                        hotkeys: { ...bindings, [entry.key]: value },
                      }))
                    }
                  />
                  {conflict ? (
                    <Badge variant="destructive" className="destructive max-w-full truncate" title={conflict}>
                      {conflict}
                    </Badge>
                  ) : null}
                </div>
                <FieldError message={errors[`hotkeys.${entry.key}`]} />
              </div>
            );
          })}
        </div>
        <div className="flex flex-col gap-2 border-t border-foreground/10 pt-3">
          <div>
            <p className="text-sm font-medium">Presets</p>
            <p className="text-xs text-foreground/60">Restore a familiar shortcut layout.</p>
          </div>
        <ModifierPicker
          selected={draft.modifier}
          onSelect={(id) =>
            setDraft((d) => ({
              ...d,
              modifier: id,
              hotkeys: bindingsForModifier(id),
            }))
          }
        />
        </div>
      </CardContent>
    </Card>
  );
}
