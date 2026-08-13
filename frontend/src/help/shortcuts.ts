/**
 * Help surface data (spec §5.3): the fixed global shortcut set grouped by
 * Volume / Commands, rendered for the SELECTED modifier. Combos reuse the
 * restricted-recorder key set from Settings (modifierOptions.ts); CapsLock
 * shows the Ctrl+Alt fallback combos with a note.
 */

import { modifierById } from "../settings/modifierOptions";

export interface HelpShortcut {
  action: string;
  label: string;
  combo: string;
}

export interface HelpGroup {
  id: string;
  title: string;
  items: HelpShortcut[];
}

const VOLUME_ACTIONS = ["VolumeUp", "VolumeDown", "VolumeUpLarge", "VolumeDownLarge"];
const COMMAND_ACTIONS = ["ToggleMute", "OpenMenu", "Reset50", "OpenMixer"];

/** Grouped shortcuts for the configured modifier (CapsLock → Ctrl+Alt fallback). */
export function helpGroups(modifier: string): HelpGroup[] {
  // CapsLock is unsupported by the backend; show the Ctrl+Alt combos it falls back to.
  const mod = modifier === "CapsLock" ? modifierById("CtrlAlt") : modifierById(modifier);
  const byAction = new Map(mod.combos.map((c) => [c.action, c]));
  const pick = (action: string): HelpShortcut => {
    const combo = byAction.get(action);
    if (!combo) {
      // Defensive: never render a card without its combo.
      return { action, label: action, combo: "" };
    }
    return { action, label: combo.label, combo: combo.combo };
  };
  return [
    { id: "volume", title: "Volume", items: VOLUME_ACTIONS.map(pick) },
    { id: "commands", title: "Commands", items: COMMAND_ACTIONS.map(pick) },
  ];
}

/** Shown under the title when the configured modifier falls back to Ctrl+Alt. */
export function helpNote(modifier: string): string | null {
  return modifier === "CapsLock" ? "Caps Lock falls back to Ctrl + Alt" : null;
}
