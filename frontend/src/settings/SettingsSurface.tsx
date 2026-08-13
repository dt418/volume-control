import { useEffect, useState } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { invoke, listen } from "../lib/ipc";
import {
  type HotkeyRegResult,
  KeyCard,
  modifierForId,
} from "./KeyCard";
import { ModifierPicker } from "./ModifierPicker";

interface SettingsAppearance {
  theme: string;
  material: string;
  motion: string;
  accent: string;
}

interface SettingsConfig {
  volume_step: number;
  volume_step_large: number;
  modifier: string;
  appearance: SettingsAppearance;
}

interface BootstrapPayload {
  config: SettingsConfig;
  hotkey_status: HotkeyRegResult[];
}

const THEME_OPTIONS = ["System", "Light", "Dark"];
const MATERIAL_OPTIONS = ["Auto", "Translucent", "Opaque"];
const MOTION_OPTIONS = ["Full", "Reduced", "Disabled"];
const ACCENT_OPTIONS = ["System", "Blue", "Green", "Purple", "Orange"];

const SELECT_CLASS =
  "h-8 rounded-md border border-foreground/20 bg-background px-2 text-sm shadow-sm " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent";

export function SettingsSurface() {
  const [config, setConfig] = useState<SettingsConfig | null>(null);
  const [hotkeyStatus, setHotkeyStatus] = useState<HotkeyRegResult[]>([]);

  useEffect(() => {
    let disposed = false;
    void invoke<BootstrapPayload>("get_bootstrap")
      .then((payload) => {
        if (disposed) return;
        setConfig(payload.config);
        setHotkeyStatus(payload.hotkey_status);
      })
      .catch(() => {
        if (!disposed) setConfig(null);
      });

    // Live conflict updates: registration status changes after set_modifier
    // or an external conflict appear here without a reload.
    let unlisten: (() => void) | undefined;
    void listen<HotkeyRegResult[]>("state://hotkeys", (status) => {
      if (!disposed) setHotkeyStatus(status);
    }).then((fn) => {
      unlisten = fn;
    });

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        void invoke("close_surface", { surface: "window-settings" });
      }
    };
    window.addEventListener("keydown", onKeyDown);

    return () => {
      disposed = true;
      window.removeEventListener("keydown", onKeyDown);
      unlisten?.();
    };
  }, []);

  if (!config) {
    return <main className="p-4 text-sm text-foreground/80">Loading…</main>;
  }

  const setModifier = (id: string) => {
    setConfig((prev) => (prev ? { ...prev, modifier: id } : prev));
    void invoke("set_modifier", { modifier: id });
  };

  const patchConfig = (patch: Record<string, unknown>) => {
    void invoke("update_settings", { patch });
  };

  const commitStep = (field: "volume_step" | "volume_step_large", raw: string) => {
    const value = Number.parseInt(raw, 10);
    if (Number.isNaN(value)) return;
    setConfig((prev) => (prev ? { ...prev, [field]: value } : prev));
    patchConfig({ [field]: value });
  };

  const commitAppearance = (field: keyof SettingsAppearance, value: string) => {
    setConfig((prev) =>
      prev
        ? { ...prev, appearance: { ...prev.appearance, [field]: value } }
        : prev,
    );
    patchConfig({ [field]: value });
  };

  const modifier = modifierForId(config.modifier);

  return (
    <main className="flex flex-col gap-4 p-4">
      <Card>
        <CardHeader>
          <CardTitle>Hotkey modifier</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <ModifierPicker selected={config.modifier} onSelect={setModifier} />
          <KeyCard modifier={modifier} hotkeyStatus={hotkeyStatus} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Volume steps</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <label className="flex items-center justify-between gap-2 text-sm">
            <span className="text-foreground/80">Volume step (%)</span>
            <Input
              type="number"
              min={1}
              max={100}
              aria-label="Volume step"
              className="w-20"
              defaultValue={config.volume_step}
              onBlur={(event) =>
                commitStep("volume_step", event.target.value)
              }
            />
          </label>
          <label className="flex items-center justify-between gap-2 text-sm">
            <span className="text-foreground/80">Large volume step (%)</span>
            <Input
              type="number"
              min={1}
              max={100}
              aria-label="Large volume step"
              className="w-20"
              defaultValue={config.volume_step_large}
              onBlur={(event) =>
                commitStep("volume_step_large", event.target.value)
              }
            />
          </label>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Appearance</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <AppearanceRow
            label="Theme"
            options={THEME_OPTIONS}
            value={config.appearance.theme}
            onChange={(v) => commitAppearance("theme", v)}
          />
          <AppearanceRow
            label="Material"
            options={MATERIAL_OPTIONS}
            value={config.appearance.material}
            onChange={(v) => commitAppearance("material", v)}
          />
          <AppearanceRow
            label="Motion"
            options={MOTION_OPTIONS}
            value={config.appearance.motion}
            onChange={(v) => commitAppearance("motion", v)}
          />
          <AppearanceRow
            label="Accent"
            options={ACCENT_OPTIONS}
            value={config.appearance.accent}
            onChange={(v) => commitAppearance("accent", v)}
          />
        </CardContent>
      </Card>
    </main>
  );
}

function AppearanceRow({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: string[];
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="flex items-center justify-between gap-2 text-sm">
      <span className="text-foreground/80">{label}</span>
      <select
        aria-label={label}
        className={SELECT_CLASS}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      >
        {options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
    </label>
  );
}
