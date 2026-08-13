import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import type { DraftSetter, SettingsConfig } from "./settingsTypes";
import { KeyCard, type HotkeyRegResult, modifierForId } from "./KeyCard";
import { ModifierPicker } from "./ModifierPicker";

/** Hotkeys section (legacy parity): the restricted modifier picker (draft) +
 *  the read-only key card with live conflict badges. */
export function HotkeysSection({
  draft,
  setDraft,
  hotkeyStatus,
}: {
  draft: SettingsConfig;
  setDraft: DraftSetter;
  hotkeyStatus: HotkeyRegResult[];
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Hotkey modifier</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <ModifierPicker
          selected={draft.modifier}
          onSelect={(id) => setDraft((d) => ({ ...d, modifier: id }))}
        />
        <KeyCard modifier={modifierForId(draft.modifier)} hotkeyStatus={hotkeyStatus} />
      </CardContent>
    </Card>
  );
}
