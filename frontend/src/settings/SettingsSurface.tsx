import { useEffect, useState } from "react";

import { applyAppearance, type AppearancePayload } from "../lib/appearance";
import { invoke, listen } from "../lib/ipc";
import type { HotkeyRegResult } from "./KeyCard";
import { AppearanceSection } from "./AppearanceSection";
import { BlacklistEditor } from "./BlacklistEditor";
import { FeedbackSection } from "./FeedbackSection";
import { GeneralSection } from "./GeneralSection";
import { HotkeysSection } from "./HotkeysSection";
import { SectionNav, type SettingsSection } from "./SectionNav";
import { StorageSection } from "./StorageSection";
import type { FieldErrors, SettingsConfig } from "./settingsTypes";

interface BootstrapPayload {
  config: SettingsConfig;
  hotkey_status: HotkeyRegResult[];
  appearance: AppearancePayload;
}

/** Backend error field name → the section that owns it (legacy: a failed
 *  Apply switches to the section containing the offending field). */
const SECTION_FOR_FIELD: Record<string, SettingsSection> = {
  volume_step: "General",
  volume_step_large: "General",
  overlay_duration_ms: "General",
  modifier: "Hotkeys",
  "beep.enabled": "Feedback",
  "beep.blocked_freq": "Feedback",
  "beep.blocked_duration_ms": "Feedback",
  "beep.limit_freq": "Feedback",
  "beep.limit_duration_ms": "Feedback",
  "color_thresholds.green_up_to": "Appearance",
  "color_thresholds.blue_up_to": "Appearance",
  "color_thresholds.orange_up_to": "Appearance",
  blacklist: "Blacklist",
};

/** Backend errors render as "field: message" (config::ConfigValidationError
 *  Display). Extract the field so the surface can place an inline error and
 *  switch to the owning section. */
function fieldFromMessage(message: string): string | null {
  const match = /^([a-z][a-z_.]*): /.exec(message);
  return match ? match[1] : null;
}

/** Build ONE SettingsPatch from the draft (atomic commit; the backend only
 *  applies present fields). Modifier is committed separately via set_modifier
 *  after a successful save. */
function buildPatch(draft: SettingsConfig): Record<string, unknown> {
  return {
    volume_step: draft.volume_step,
    volume_step_large: draft.volume_step_large,
    overlay_duration_ms: draft.overlay_duration_ms,
    theme: draft.appearance.theme,
    material: draft.appearance.material,
    motion: draft.appearance.motion,
    accent: draft.appearance.accent,
    beep: { ...draft.beep },
    color_thresholds: { ...draft.color_thresholds },
    blacklist: draft.blacklist,
  };
}

/** Six-section Settings surface with the legacy draft lifecycle: edits stay
 *  in the draft until "Save changes"; Reset restores the committed config;
 *  Cancel discards and closes. A rejected save retains the edits, shows an
 *  inline field error, and switches to the section owning the field. */
export function SettingsSurface() {
  const [config, setConfig] = useState<SettingsConfig | null>(null);
  const [draft, setDraft] = useState<SettingsConfig | null>(null);
  const [hotkeyStatus, setHotkeyStatus] = useState<HotkeyRegResult[]>([]);
  const [activeSection, setActiveSection] = useState<SettingsSection>("General");
  const [fieldErrors, setFieldErrors] = useState<FieldErrors>({});
  const [status, setStatus] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let disposed = false;
    void invoke<BootstrapPayload>("get_bootstrap")
      .then((payload) => {
        if (disposed) return;
        applyAppearance(payload.appearance);
        setConfig(payload.config);
        setDraft(payload.config);
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

  if (!config || !draft) {
    return <main className="p-4 text-sm text-foreground/80">Loading…</main>;
  }

  const dirty =
    JSON.stringify(config) !== JSON.stringify(draft);

  const updateDraft = (update: (d: SettingsConfig) => SettingsConfig) => {
    setDraft((prev) => (prev ? update(prev) : prev));
    setStatus(null);
  };

  const save = async () => {
    setSaving(true);
    setFieldErrors({});
    setStatus(null);
    try {
      await invoke("update_settings", { patch: buildPatch(draft) });
      if (draft.modifier !== config.modifier) {
        await invoke("set_modifier", { modifier: draft.modifier });
      }
      setConfig(draft);
      setStatus("Saved");
    } catch (error) {
      const message = String(error);
      const field = fieldFromMessage(message);
      if (field) {
        setFieldErrors({ [field]: message });
        const section = SECTION_FOR_FIELD[field];
        if (section) setActiveSection(section);
      } else {
        setStatus(message);
      }
    } finally {
      setSaving(false);
    }
  };

  const reset = () => {
    setDraft(config);
    setFieldErrors({});
    setStatus(null);
  };

  const cancel = () => {
    void invoke("close_surface", { surface: "window-settings" });
  };

  return (
    <main className="flex min-h-screen flex-col gap-4 bg-background p-4">
      <SectionNav active={activeSection} onSelect={setActiveSection} />

      <div className="flex flex-1 flex-col gap-4">
        {activeSection === "General" && (
          <GeneralSection draft={draft} setDraft={updateDraft} errors={fieldErrors} />
        )}
        {activeSection === "Hotkeys" && (
          <HotkeysSection draft={draft} setDraft={updateDraft} hotkeyStatus={hotkeyStatus} />
        )}
        {activeSection === "Appearance" && (
          <AppearanceSection draft={draft} setDraft={updateDraft} errors={fieldErrors} />
        )}
        {activeSection === "Blacklist" && (
          <BlacklistEditor draft={draft} setDraft={updateDraft} />
        )}
        {activeSection === "Feedback" && (
          <FeedbackSection draft={draft} setDraft={updateDraft} errors={fieldErrors} />
        )}
        {activeSection === "Storage" && <StorageSection />}
      </div>

      <footer className="sticky bottom-0 flex items-center justify-between gap-2 border-t border-foreground/10 bg-background/90 py-2">
        <p
          role="status"
          className="text-sm text-foreground/70"
          aria-live="polite"
        >
          {status ?? "\u00a0"}
        </p>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={reset}
            disabled={!dirty}
            className="rounded-md border border-foreground/20 px-3 py-1.5 text-sm disabled:opacity-40"
          >
            Reset
          </button>
          <button
            type="button"
            onClick={cancel}
            className="rounded-md border border-foreground/20 px-3 py-1.5 text-sm"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => void save()}
            disabled={!dirty || saving}
            className="rounded-md bg-accent px-3 py-1.5 text-sm font-medium text-accent-foreground disabled:opacity-40"
          >
            Save changes
          </button>
        </div>
      </footer>
    </main>
  );
}
