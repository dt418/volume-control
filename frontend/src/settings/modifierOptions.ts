/**
 * Restricted hotkey recorder data (spec D4): the user may only change the
 * modifier; the fixed key set is shown as read-only key cards. Combos are
 * rendered for the SELECTED modifier.
 */

export type ModifierId = "CtrlAlt" | "Alt" | "Ctrl" | "CapsLock";

export interface ComboDisplay {
  /** HotkeyAction serialized name (matches bootstrap.hotkey_status action). */
  action: string;
  /** Friendly label shown next to the combo. */
  label: string;
  /** Full combo string for the modifier, e.g. "Ctrl+Alt+↑". */
  combo: string;
}

export interface ModifierOption {
  id: ModifierId;
  label: string;
  /** Full combos shown as key cards when this modifier is selected. */
  combos: ComboDisplay[];
  /** True when the option cannot be selected (CapsLock → Ctrl+Alt fallback). */
  disabled?: boolean;
  disabledReason?: string;
}

function combos(base: string): ComboDisplay[] {
  return [
    { action: "VolumeUp", label: "Volume Up", combo: `${base}+↑` },
    { action: "VolumeDown", label: "Volume Down", combo: `${base}+↓` },
    { action: "VolumeUpLarge", label: "Volume Up (large)", combo: `${base}+Shift+↑` },
    { action: "VolumeDownLarge", label: "Volume Down (large)", combo: `${base}+Shift+↓` },
    { action: "ToggleMute", label: "Toggle Mute", combo: `${base}+M` },
    { action: "OpenMenu", label: "Open Menu", combo: `${base}+Shift+M` },
    { action: "Reset50", label: "Reset to 50%", combo: `${base}+R` },
    { action: "OpenMixer", label: "Open Mixer", combo: `${base}+V` },
  ];
}

export const MODIFIER_OPTIONS: ModifierOption[] = [
  { id: "CtrlAlt", label: "Ctrl + Alt", combos: combos("Ctrl+Alt") },
  { id: "Alt", label: "Alt", combos: combos("Alt") },
  { id: "Ctrl", label: "Ctrl", combos: combos("Ctrl") },
  {
    id: "CapsLock",
    label: "Caps Lock",
    combos: [],
    disabled: true,
    disabledReason: "Falls back to Ctrl + Alt",
  },
];

export function modifierById(id: string): ModifierOption {
  return (
    MODIFIER_OPTIONS.find((m) => m.id === id) ?? MODIFIER_OPTIONS[0]
  );
}
