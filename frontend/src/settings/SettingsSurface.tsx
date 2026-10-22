import { useEffect, useMemo, useState } from "react";

import { SurfaceCloseButton } from "../components/ui/surface-close-button";
import { asAppLabelStyle } from "../lib/appLabel";
import { applyAppearance, type AppearancePayload } from "../lib/appearance";
import { asLanguage, LanguageProvider, translatorFor } from "../lib/i18n";
import { invoke, listen } from "../lib/ipc";
import { markSurfaceReady, surfaceErrorMessage } from "../lib/surface";
import type { HotkeyRegResult } from "./KeyCard";
import { AppearanceSection } from "./AppearanceSection";
import { BlacklistEditor } from "./BlacklistEditor";
import { FeedbackSection } from "./FeedbackSection";
import { GeneralSection } from "./GeneralSection";
import { HotkeysSection } from "./HotkeysSection";
import { bindingsForModifier } from "./modifierOptions";
import { SectionNav, type SettingsSection } from "./SectionNav";
import { StorageSection, type ConfigLoadNotice } from "./StorageSection";
import type { FieldErrors, SettingsConfig } from "./settingsTypes";

interface BootstrapPayload {
  config: SettingsConfig;
  config_notice?: ConfigLoadNotice;
  hotkey_status: HotkeyRegResult[];
  appearance: AppearancePayload;
}

interface AutostartStatus {
  enabled: boolean;
  command: string | null;
}

/** Backend error field name → the section that owns it (legacy: a failed
 *  Apply switches to the section containing the offending field). */
const SECTION_FOR_FIELD: Record<string, SettingsSection> = {
  volume_step: "General",
  volume_step_large: "General",
  overlay_duration_ms: "General",
  app_label_style: "General",
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
  const match = /^([a-z_][a-z0-9_.]*): /.exec(message);
  return match ? match[1] : null;
}

function hydrateConfig(config: SettingsConfig): SettingsConfig {
  return {
    ...config,
    language: asLanguage(config.language),
    app_label_style: asAppLabelStyle(config.app_label_style),
    hotkeys: config.hotkeys ?? bindingsForModifier(config.modifier),
  };
}

/** Build ONE SettingsPatch from the draft (atomic commit; the backend only
 *  applies present fields). Modifier is committed separately via set_modifier
 *  after a successful save. */
function buildPatch(draft: SettingsConfig): Record<string, unknown> {
  return {
    volume_step: draft.volume_step,
    volume_step_large: draft.volume_step_large,
    overlay_duration_ms: draft.overlay_duration_ms,
    hotkeys: { ...(draft.hotkeys ?? bindingsForModifier(draft.modifier)) },
    theme: draft.appearance.theme,
    material: draft.appearance.material,
    motion: draft.appearance.motion,
    accent: draft.appearance.accent,
    beep: { ...draft.beep },
    color_thresholds: { ...draft.color_thresholds },
    blacklist: draft.blacklist,
    language: draft.language,
    app_label_style: draft.app_label_style,
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
  const [error, setError] = useState<string | null>(null);
  const [autostartDisabled, setAutostartDisabled] = useState(false);
  const [configNotice, setConfigNotice] = useState<ConfigLoadNotice | null>(null);
  const language = asLanguage(draft?.language ?? config?.language);
  const tr = useMemo(() => translatorFor(language), [language]);

  useEffect(() => {
    const title = tr("app.window.settings");
    document.title = title;
    void invoke<void>("set_window_title", { title }).catch(() => {});
  }, [tr]);

  useEffect(() => {
    let disposed = false;
    void invoke<BootstrapPayload>("get_bootstrap")
      .then((payload) => {
        if (disposed) return;
        setError(null);
        applyAppearance(payload.appearance);
        const hydrated = hydrateConfig(payload.config);
        setConfig(hydrated);
        setDraft(hydrated);
        setHotkeyStatus(payload.hotkey_status);
        setConfigNotice(payload.config_notice ?? null);
        void invoke<AutostartStatus>("get_autostart")
          .then((autostart) => {
            if (!disposed) setAutostartDisabled(false);
          })
          .catch(() => {
            if (!disposed) setAutostartDisabled(true);
          });
      })
      .catch((reason) => {
        if (!disposed) {
          setConfig(null);
          setDraft(null);
          setError(surfaceErrorMessage(reason));
        }
      })
      .finally(() => {
        if (!disposed) void markSurfaceReady();
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

  const dirty = Boolean(config && draft && JSON.stringify(config) !== JSON.stringify(draft));

  const updateDraft = (update: (d: SettingsConfig) => SettingsConfig) => {
    setDraft((prev) => (prev ? update(prev) : prev));
    setStatus(null);
  };

  const save = async () => {
    if (!config || !draft) return;
    setSaving(true);
    setFieldErrors({});
    setStatus(null);
    try {
      await invoke("update_settings", { patch: buildPatch(draft) });
      if (draft.modifier !== config.modifier) {
        await invoke("set_modifier", { modifier: draft.modifier });
      }
      if (Boolean(draft.autostart) !== Boolean(config.autostart)) {
        await invoke("set_autostart", { enabled: Boolean(draft.autostart) });
      }
      setConfig(draft);
      setStatus(tr("status.saved"));
    } catch (error) {
      const message = String(error);
      const field = fieldFromMessage(message);
      if (field) {
        setFieldErrors({ [field]: message });
        const section = field.startsWith("hotkeys.") ? "Hotkeys" : SECTION_FOR_FIELD[field];
        if (section) setActiveSection(section);
      } else {
        setStatus(message);
      }
    } finally {
      setSaving(false);
    }
  };

  const reset = () => {
    if (!config) return;
    setDraft(config);
    setFieldErrors({});
    setStatus(null);
  };

  const cancel = () => {
    void invoke("close_surface", { surface: "window-settings" });
  };

  const changeAutostart = (enabled: boolean) => {
    updateDraft((d) => ({ ...d, autostart: enabled }));
  };

  return (
    <LanguageProvider language={language}>
    <main
      data-surface="settings"
      className="surface-shell h-dvh min-h-0 overflow-hidden flex flex-col gap-3 bg-background p-3"
    >
      <header
        data-testid="surface-header"
        className="flex flex-shrink-0 items-start justify-between border-b border-foreground/10 pb-2"
      >
        <div>
          <h1 className="text-base font-semibold">{tr("app.window.settings")}</h1>
          <p className="text-xs text-foreground/60">{tr("settings.subtitle")}</p>
        </div>
        <SurfaceCloseButton label={tr("common.close")} onClick={cancel} />
      </header>

      <div
        data-testid="surface-content"
        className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden min-[760px]:flex-row"
      >
        {error ? (
          <div className="surface-scroll min-h-0 flex-1 overflow-y-auto">
            <div role="alert" className="rounded-md border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
              {tr("settings.loadFailed")}: {error}
            </div>
          </div>
        ) : !config || !draft ? (
          <div className="surface-scroll min-h-0 flex-1 overflow-y-auto">
            <p className="p-2 text-sm text-foreground/70">Loading…</p>
          </div>
        ) : (
          <>
            <SectionNav active={activeSection} onSelect={setActiveSection} />
            <div className="surface-scroll min-h-0 flex-1 overflow-y-auto pr-1">
              {activeSection === "General" && (
                <GeneralSection
                  draft={draft}
                  setDraft={updateDraft}
                  errors={fieldErrors}
                  autoStartEnabled={Boolean(draft.autostart)}
                  autoStartDisabled={autostartDisabled}
                  onAutoStartChange={changeAutostart}
                />
              )}
              {activeSection === "Hotkeys" && (
                <HotkeysSection
                  draft={draft}
                  setDraft={updateDraft}
                  hotkeyStatus={hotkeyStatus}
                  errors={fieldErrors}
                />
              )}
              {activeSection === "Appearance" && (
                <AppearanceSection draft={draft} setDraft={updateDraft} errors={fieldErrors} />
              )}
              {activeSection === "Blacklist" && (
                <BlacklistEditor draft={draft} setDraft={updateDraft} saved={config.blacklist} />
              )}
              {activeSection === "Feedback" && (
                <FeedbackSection draft={draft} setDraft={updateDraft} errors={fieldErrors} />
              )}
              {activeSection === "Storage" && <StorageSection notice={configNotice} />}
            </div>
          </>
        )}
      </div>

      <footer data-testid="surface-footer" className="flex flex-shrink-0 items-center justify-between gap-2 border-t border-foreground/10 pt-2">
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
            disabled={!dirty || !draft}
            className="rounded-md border border-foreground/20 px-3 py-1.5 text-sm transition-colors hover:bg-foreground/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-40"
          >
            {tr("common.reset")}
          </button>
          <button
            type="button"
            onClick={cancel}
            className="rounded-md border border-foreground/20 px-3 py-1.5 text-sm transition-colors hover:bg-foreground/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            {tr("common.cancel")}
          </button>
          <button
            type="button"
            onClick={() => void save()}
            disabled={!dirty || saving || !draft}
            className="rounded-md bg-accent px-3 py-1.5 text-sm font-medium text-accent-foreground transition-colors hover:bg-accent/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:opacity-40"
          >
            {tr("common.saveChanges")}
          </button>
        </div>
      </footer>
    </main>
    </LanguageProvider>
  );
}
