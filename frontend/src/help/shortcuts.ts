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
 *
 * Every user-visible string comes from the translator; the parameter defaults
 * to English so data-only callers and tests stay language-agnostic.
 */

import { translatorFor, type Translator } from "../lib/i18n";
import {
  bindingsForModifier,
  formatShortcut,
  HOTKEY_ACTIONS,
  type HotkeyActionKey,
} from "../settings/modifierOptions";
import type { HotkeyBindings } from "../settings/settingsTypes";

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

/** Grouped shortcuts for the configured bindings. The string overload keeps
 * old callers/tests working while a fresh config uses the recorded map. */
export function helpGroups(
  source: string | HotkeyBindings,
  tr: Translator = translatorFor("en"),
): HelpGroup[] {
  const bindings = typeof source === "string" ? bindingsForModifier(source) : source;
  const pick = (action: string): HelpShortcut => {
    const entry = HOTKEY_ACTIONS.find((candidate) => candidate.action === action);
    const label = entry ? tr(entry.labelKey) : action;
    const combo = entry ? bindings[entry.key as HotkeyActionKey] : "";
    // Defensive: never render a card without its combo.
    return { action, label, combo: combo ? formatShortcut(combo) : "" };
  };
  return [
    {
      id: "volume",
      title: tr("help.groupVolume"),
      items: BASE_ACTIONS.slice(0, 2).map(pick),
    },
    {
      id: "commands",
      title: tr("help.groupCommands"),
      items: BASE_ACTIONS.slice(2).map(pick),
    },
    {
      id: "extended",
      title: tr("help.groupExtended"),
      items: EXTENDED_ACTIONS.map(pick),
    },
  ];
}

/** The five base actions (used for status badges + the conflict callout). */
export function baseActions(
  source: string | HotkeyBindings,
  tr: Translator = translatorFor("en"),
): HelpShortcut[] {
  return helpGroups(source, tr)
    .filter((g) => g.id !== "extended")
    .flatMap((g) => g.items);
}

/** Shown under the title when the configured modifier falls back to Ctrl+Alt. */
export function helpNote(
  modifier: string,
  tr: Translator = translatorFor("en"),
): string | null {
  return modifier === "CapsLock" ? tr("help.capsFallback") : null;
}
