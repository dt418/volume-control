import type { HotkeyBindings } from "./settingsTypes";

/** Shared action metadata for the recorder rows and the Help surface. */
export const HOTKEY_ACTIONS = [
  { key: "volume_up", action: "VolumeUp", label: "Volume Up", help: "Increase volume by the small step." },
  { key: "volume_down", action: "VolumeDown", label: "Volume Down", help: "Decrease volume by the small step." },
  { key: "volume_up_large", action: "VolumeUpLarge", label: "Volume Up (large)", help: "Increase volume by the large step." },
  { key: "volume_down_large", action: "VolumeDownLarge", label: "Volume Down (large)", help: "Decrease volume by the large step." },
  { key: "toggle_mute", action: "ToggleMute", label: "Toggle Mute", help: "Mute or unmute the current output." },
  { key: "reset_50", action: "Reset50", label: "Reset to 50%", help: "Set the current output to 50%." },
  { key: "open_mixer", action: "OpenMixer", label: "Open Mixer", help: "Show or hide the mixer panel." },
  { key: "open_menu", action: "OpenMenu", label: "Open Menu", help: "Open the VolumeControl tray menu." },
] as const;

export type HotkeyActionKey = (typeof HOTKEY_ACTIONS)[number]["key"];

/** Convert the portable persisted binding map into the current preset. */
export function bindingsForModifier(modifier: string): HotkeyBindings {
  const base = modifier === "Alt" ? "Alt" : modifier === "Ctrl" ? "Ctrl" : "Ctrl+Alt";
  return {
    volume_up: `${base}+ArrowUp`,
    volume_down: `${base}+ArrowDown`,
    volume_up_large: `${base}+Shift+ArrowUp`,
    volume_down_large: `${base}+Shift+ArrowDown`,
    toggle_mute: `${base}+KeyM`,
    reset_50: `${base}+KeyR`,
    open_mixer: `${base}+KeyV`,
    open_menu: `${base}+Shift+KeyM`,
  };
}

/** Turn a stored Code-style shortcut into a compact, screenshot-like label. */
export function formatShortcut(value: string): string {
  return value
    .split("+")
    .map((token) => {
      const normalized = token.trim();
      const labels: Record<string, string> = {
        Control: "Ctrl",
        Ctrl: "Ctrl",
        Option: "Alt",
        Alt: "Alt",
        Command: "⌘",
        Cmd: "⌘",
        Super: "⌘",
        Shift: "Shift",
        ArrowUp: "↑",
        ArrowDown: "↓",
        ArrowLeft: "←",
        ArrowRight: "→",
        Space: "Space",
        Escape: "Esc",
        Backspace: "Backspace",
        Delete: "Delete",
        Enter: "Enter",
        Tab: "Tab",
      };
      if (labels[normalized]) return labels[normalized];
      if (/^Key[A-Z]$/.test(normalized)) return normalized.slice(3);
      if (/^Digit[0-9]$/.test(normalized)) return normalized.slice(5);
      return normalized;
    })
    .join("+");
}

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
