/**
 * Help surface data (spec §5.3): the fixed global shortcut set rendered for
 * the SELECTED modifier. Combos reuse the restricted-recorder key set from
 * Settings (modifierOptions.ts); CapsLock shows the Ctrl+Alt fallback combos
 * with a note.
 *
 * Legacy parity (help.rs): the five BASE actions form the primary rows with
 * per-row registration status pills; the shift variants are a separate
 * extended section (the legacy card deliberately documented only base
 * combos, with shift variants sharing the base action's status).
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

/** The five legacy base actions (help.rs ACTION_ROWS labels, spec §8.2). */
const BASE_ACTIONS = [
  "VolumeUp",
  "VolumeDown",
  "ToggleMute",
  "OpenMixer",
  "Reset50",
] as const;

/** Extended webview additions: the shift variants + the menu action. */
const EXTENDED_ACTIONS = ["VolumeUpLarge", "VolumeDownLarge", "OpenMenu"] as const;

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
    { id: "volume", title: "Volume", items: BASE_ACTIONS.slice(0, 2).map(pick) },
    {
      id: "commands",
      title: "Commands",
      items: BASE_ACTIONS.slice(2).map(pick),
    },
    {
      id: "extended",
      title: "Extended",
      items: EXTENDED_ACTIONS.map(pick),
    },
  ];
}

/** The five base actions (used for status badges + the conflict callout). */
export function baseActions(modifier: string): HelpShortcut[] {
  return helpGroups(modifier)
    .filter((g) => g.id !== "extended")
    .flatMap((g) => g.items);
}

/** Shown under the title when the configured modifier falls back to Ctrl+Alt. */
export function helpNote(modifier: string): string | null {
  return modifier === "CapsLock" ? "Caps Lock falls back to Ctrl + Alt" : null;
}
