//! Cross-platform application core — the single source of truth (SSOT).
//!
//! [`AppCore`] owns the config, the audio backend, the global hotkey
//! listener, and the last confirmed volume state, and applies every
//! [`AppAction`] emitted by any surface. It is deliberately free of native
//! surface plumbing (overlay/tray/webview windows): the host routes surface
//! open/close through the [`EventSink`], and every confirmed volume change is
//! pushed to the host as a `state://volume` event. The Tauri host wires the
//! sink in `src-tauri`: it renders the tray and the HUD overlay natively on
//! Windows (`native_win32`) and through a Tauri tray plus a webview overlay
//! window on Linux/macOS (`tauri_tray`, `events_sink`).

use std::sync::Arc;

use serde::Serialize;

use crate::audio::{AudioBackend, VolumeState};
use crate::config::{Config, HotkeyBindings, HotkeyModifier};
use crate::hotkeys::{HotkeyAction, HotkeyRegResult};
use crate::hotkeys_global::GlobalHotkeys;
use crate::ui::{AccentMode, AppAction, MaterialMode, MotionMode, SurfaceId, ThemeMode};

/// Partial settings patch from the Settings webview. Every field is
/// optional; absent fields are left unchanged. `update_settings` mutates the
/// in-memory config only — the caller (command layer) persists via
/// [`AppCore::save_config`] so tests never touch the user's config file.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SettingsPatch {
    pub volume_step: Option<u32>,
    pub volume_step_large: Option<u32>,
    /// Enum variant names: "System" | "Light" | "Dark".
    pub theme: Option<String>,
    /// Enum variant names: "Auto" | "Translucent" | "Opaque".
    pub material: Option<String>,
    /// Enum variant names: "Full" | "Reduced" | "Disabled".
    pub motion: Option<String>,
    /// Enum variant names: "System" | "Blue" | "Green" | "Purple" | "Orange".
    pub accent: Option<String>,
    /// Overlay visible time in ms (200..=10_000).
    pub overlay_duration_ms: Option<u64>,
    /// Full shortcut map recorded in Settings. The backend validates every
    /// entry and only adopts the map after it has been persisted safely.
    pub hotkeys: Option<HotkeyBindings>,
    /// Beep feedback fields (all optional within the group).
    pub beep: Option<BeepPatch>,
    /// Colour legend thresholds (all optional within the group).
    pub color_thresholds: Option<ColorThresholdsPatch>,
    /// UI language: "en" | "vi".
    pub language: Option<String>,
    /// App-name label style: "friendly" | "friendly_and_process" | "process".
    pub app_label_style: Option<String>,
    /// Full-list replace for the blacklist (draft commits the whole list
    /// atomically; entries are normalized per-platform).
    pub blacklist: Option<Vec<String>>,
    /// Persisted auto-start preference. The registry side effect is handled by
    /// [`crate::autostart`]; this field keeps the typed config in sync for
    /// bootstrap and migration compatibility.
    pub autostart: Option<bool>,
    /// Base UI font size in logical points (9..=24).
    pub ui_font_size: Option<u8>,
}

/// Optional beep feedback patch (mirrors [`crate::config::BeepConfig`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BeepPatch {
    pub enabled: Option<bool>,
    pub blocked_freq: Option<u32>,
    pub blocked_duration_ms: Option<u32>,
    pub limit_freq: Option<u32>,
    pub limit_duration_ms: Option<u32>,
}

/// Optional colour-threshold patch (mirrors
/// [`crate::config::ColorThresholds`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ColorThresholdsPatch {
    pub green_up_to: Option<u8>,
    pub blue_up_to: Option<u8>,
    pub orange_up_to: Option<u8>,
}

/// One application audio session in the per-app mixer. The Windows WASAPI
/// source (Task 2b) enumerates these; other platforms report an empty list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioSessionInfo {
    pub id: String,
    pub name: String,
    /// Normalized executable/base process name, independent of the WASAPI
    /// display name. Process-only labels use this value.
    pub process: String,
    /// Human-friendly product name resolved from the executable's version
    /// information. `None` when the process has no readable version resource;
    /// the UI then falls back to `name`.
    pub display_name: Option<String>,
    pub pct: u8,
    pub muted: bool,
    pub active: bool,
    /// True for Windows' PID-less "System Sounds" session. The mixer uses it
    /// for the friendly label; it is never derived from the process id.
    pub system: bool,
}

/// One user-facing process with a visible top-level window. The blacklist
/// stores `process` (normalized for the platform); `title` is shown in the
/// picker so two windows of the same executable are still distinguishable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunningApp {
    pub process: String,
    pub title: String,
    /// Human-friendly product name resolved from the executable. `None` when
    /// no executable path/version resource is available.
    pub display_name: Option<String>,
    /// `data:image/png;base64,...` shell icon, or `None` when extraction
    /// failed. The picker falls back to an initial tile.
    pub icon: Option<String>,
}

/// Merge one window's process into the picker list, keeping one row per
/// process. The first non-empty window title is used.
#[cfg(any(target_os = "windows", test))]
fn merge_running_app(
    apps: &mut Vec<RunningApp>,
    process: String,
    title: String,
    display_name: Option<String>,
    icon: Option<String>,
) {
    if process.is_empty() {
        return;
    }
    if let Some(existing) = apps.iter_mut().find(|app| app.process == process) {
        if existing.title.is_empty() && !title.is_empty() {
            existing.title = title;
        }
        if existing.display_name.is_none() && display_name.is_some() {
            existing.display_name = display_name;
        }
        return;
    }
    apps.push(RunningApp {
        process,
        title,
        display_name,
        icon,
    });
}

/// Per-app audio session source behind [`AppCore`]'s session commands.
/// Windows provides a WASAPI implementation; every other platform uses
/// [`NoopSessions`] (no per-app mixing).
pub trait SessionsSource: Send + Sync {
    /// Whether the platform exposes per-app sessions (drives
    /// `BootstrapPayload::sessions_supported`).
    fn supported(&self) -> bool {
        false
    }
    /// The current session list (empty when unsupported or unavailable).
    fn list(&self) -> Vec<AudioSessionInfo>;
    /// Set one session's volume (0–100). A stale/missing id returns `Err` so
    /// the caller can re-emit the fresh list (spec §9.6).
    fn set_volume(&self, id: &str, pct: u8) -> Result<(), String>;
    /// Toggle one session's mute. Same stale-id contract as [`Self::set_volume`].
    fn mute(&self, id: &str) -> Result<(), String>;
}

/// Platform-independent fallback: no per-app sessions.
pub struct NoopSessions;

impl SessionsSource for NoopSessions {
    fn list(&self) -> Vec<AudioSessionInfo> {
        Vec::new()
    }
    fn set_volume(&self, _id: &str, _pct: u8) -> Result<(), String> {
        Ok(())
    }
    fn mute(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Resolved appearance tokens pushed to every webview surface. Values match
/// the config's serialized strings so the frontend can treat them uniformly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppearancePayload {
    /// `"dark"` | `"light"` after resolving the configured theme against the
    /// platform system theme.
    pub theme_resolved: String,
    pub material: String,
    pub motion: String,
    pub accent: String,
}

/// One-shot payload returned by `get_bootstrap` on webview mount.
#[derive(Serialize)]
pub struct BootstrapPayload {
    pub config: Config,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_notice: Option<crate::config::ConfigLoadNotice>,
    pub volume_pct: u8,
    pub muted: bool,
    pub hotkey_status: Vec<HotkeyRegResult>,
    pub appearance: AppearancePayload,
    pub sessions: Vec<AudioSessionInfo>,
    pub sessions_supported: bool,
    pub backend_status: BackendStatus,
    /// Debug E2E marker: the host disables the overlay auto-hide timers so
    /// the embedded WebDriver can attach and assert the HUD. Always false in
    /// production builds.
    pub e2e_debug: bool,
}

/// Host-facing notifications emitted by [`AppCore`]. The Tauri host
/// implements this with `app.emit("state://*", ...)` and routes surface
/// open/close to its lazy [`WindowManager`](crate window manager).
pub trait EventSink: Send + Sync {
    /// A confirmed volume/mute change (hotkey, wheel, tray or slider).
    fn volume(&self, pct: u8, muted: bool);
    /// The per-action registration status changed (registration or a
    /// modifier change).
    fn hotkeys(&self, status: &[HotkeyRegResult]);
    /// The audio-session list changed.
    fn sessions(&self, sessions: &[AudioSessionInfo]);
    /// The audio backend health changed (ready ↔ degraded). Default no-op so
    /// existing sinks keep compiling; the Tauri host emits `state://backend`
    /// so surfaces can show a live degraded state instead of freezing
    /// silently when the backend hiccups.
    fn backend(&self, _status: &BackendStatus) {}
    /// The host should open the surface with `label` (e.g. `"window-mixer"`).
    fn open_surface(&self, _label: &str) {}
    /// The host should close/destroy the surface with `label`.
    fn close_surface(&self, _label: &str) {}
    /// Toggle the surface: open it if closed, close it if open. Defaults to
    /// open so sinks that predate the toggle seam keep their behavior.
    fn toggle_surface(&self, label: &str) {
        self.open_surface(label);
    }
    /// Show the native HUD overlay. `text: None` renders the volume HUD;
    /// `Some` renders a short text card (e.g. "Config reloaded"). The state
    /// and config are passed along so the host renderer never needs to lock
    /// the core (avoids re-entrant locking from within `handle_action`).
    /// No-op on hosts without a native overlay.
    fn overlay(&self, _text: Option<String>, _state: VolumeState, _config: Config) {}
    /// Open the native tray menu (the platform host may need a foreground
    /// unlock first).
    fn show_tray_menu(&self) {}
    /// The user asked to quit — the host tears down and exits.
    fn exit(&self) {}
}

/// Backend health published to hosts and surfaces.
///
/// `publish_confirmed_state`/`sync_external_state` used to drop the publish
/// silently on `get_state` errors, so open webviews froze on stale values
/// with no explanation ("FE/BE connection lost"). The host now emits the
/// Ready ↔ Degraded transition exactly once per change so surfaces can
/// surface the failure and recover.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum BackendStatus {
    Ready,
    Degraded { error: String },
}

/// Cross-platform application state. One instance per app, owned by the host
/// (managed as `Mutex<AppCore>` so commands can mutate it).
pub struct AppCore {
    audio: Box<dyn AudioBackend>,
    hotkeys: GlobalHotkeys,
    config: Config,
    last_state: VolumeState,
    hotkey_status: Vec<HotkeyRegResult>,
    sink: Arc<dyn EventSink>,
    sessions_source: Box<dyn SessionsSource>,
    /// Volume to restore after the host-level mute fallback drives the
    /// endpoint to zero. Native mute flags remain synchronized as well, but
    /// zeroing the scalar guarantees audible output is actually silent on
    /// devices/drivers that report a mute bit without attenuating playback.
    mute_restore_volume: Option<f32>,
    /// Last published backend health (Ready initially; degraded on the first
    /// failed probe). Drives the one-shot `state://backend` transitions.
    backend_status: BackendStatus,
    last_config_mtime: Option<std::time::SystemTime>,
    config_notice: Option<crate::config::ConfigLoadNotice>,
}

impl AppCore {
    /// Create the core: register the global hotkeys for `modifier` and take
    /// the first confirmed audio snapshot.
    pub fn new(
        audio: Box<dyn AudioBackend>,
        config: Config,
        _modifier: HotkeyModifier,
        sink: Arc<dyn EventSink>,
    ) -> Result<Self, String> {
        Self::new_inner(audio, config, _modifier, sink, None, false)
    }

    /// Create the core with an optional startup config migration/recovery notice.
    pub fn new_with_notice(
        audio: Box<dyn AudioBackend>,
        config: Config,
        _modifier: HotkeyModifier,
        sink: Arc<dyn EventSink>,
        config_notice: Option<crate::config::ConfigLoadNotice>,
    ) -> Result<Self, String> {
        Self::new_inner(audio, config, _modifier, sink, config_notice, false)
    }

    /// Create the core without touching the OS-level hotkey manager.
    ///
    /// Integration tests use this seam so a headless CI session (or
    /// concurrent Carbon registration from parallel test threads) can never
    /// abort the test process. Production hosts keep using [`Self::new`].
    #[doc(hidden)]
    pub fn new_without_native_hotkeys(
        audio: Box<dyn AudioBackend>,
        config: Config,
        _modifier: HotkeyModifier,
        sink: Arc<dyn EventSink>,
        config_notice: Option<crate::config::ConfigLoadNotice>,
    ) -> Result<Self, String> {
        Self::new_inner(audio, config, _modifier, sink, config_notice, true)
    }

    fn new_inner(
        audio: Box<dyn AudioBackend>,
        config: Config,
        _modifier: HotkeyModifier,
        sink: Arc<dyn EventSink>,
        config_notice: Option<crate::config::ConfigLoadNotice>,
        hotkeys_unavailable: bool,
    ) -> Result<Self, String> {
        let bindings = config_hotkeys(&config);
        let hotkeys = if hotkeys_unavailable {
            GlobalHotkeys::unavailable(
                &bindings,
                "native hotkey manager disabled for host-core tests",
            )
        } else {
            GlobalHotkeys::new_with_bindings(&bindings)?
        };
        // Keep the wheel-bridge modifier in sync with the initial config on
        // Windows (legacy app.rs initialized it at startup; the modifier
        // change paths below re-sync on every change). Idempotent atomic
        // store — safe regardless of when install_wheel_hook runs.
        #[cfg(target_os = "windows")]
        crate::wheel_win32::set_modifier(_modifier);
        let initial_probe = audio.get_state();
        let last_state = initial_probe.clone().unwrap_or(VolumeState {
            volume: 0.5,
            muted: false,
        });
        let backend_status = match &initial_probe {
            Ok(_) => BackendStatus::Ready,
            Err(error) => BackendStatus::Degraded {
                error: error.to_string(),
            },
        };
        let hotkey_status = hotkeys.status();
        #[cfg(target_os = "windows")]
        let sessions_source: Box<dyn SessionsSource> =
            Box::new(crate::audio_sessions_win32::WindowsSessions);
        #[cfg(target_os = "linux")]
        let sessions_source: Box<dyn SessionsSource> =
            Box::new(crate::audio_sessions_linux::PulseSessions::new());
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        let sessions_source: Box<dyn SessionsSource> = Box::new(NoopSessions);
        let core = Self {
            audio,
            hotkeys,
            config,
            last_state,
            hotkey_status,
            sink,
            sessions_source,
            mute_restore_volume: None,
            backend_status,
            last_config_mtime: config_mtime(),
            config_notice,
        };

        // Publish the first confirmed (or safe fallback) state immediately.
        // The Tauri host creates the tray before AppCore, so without this
        // startup notification the native menu remains stuck at
        // `VolumeControl — --` until the first volume mutation.
        core.sink
            .volume(core.last_state.percent(), core.last_state.muted);
        if core.backend_status != BackendStatus::Ready {
            core.sink.backend(&core.backend_status);
        }
        Ok(core)
    }

    /// Snapshot for a webview mount: current config, confirmed state,
    /// hotkey status, resolved appearance and the session list.
    pub fn bootstrap(&mut self) -> BootstrapPayload {
        let probe = self.audio.get_state();
        self.note_backend_result(&probe);
        if let Ok(st) = probe {
            self.last_state = st;
        }
        let e2e_debug =
            cfg!(debug_assertions) && std::env::var("VOLUMECTL_E2E_DEBUG").as_deref() == Ok("1");
        BootstrapPayload {
            config: self.config.clone(),
            config_notice: self.config_notice,
            volume_pct: self.last_state.percent(),
            muted: self.last_state.muted,
            hotkey_status: self.hotkey_status.clone(),
            appearance: self.appearance_payload(),
            sessions: self.sessions(),
            sessions_supported: self.sessions_source.supported(),
            backend_status: self.backend_status.clone(),
            e2e_debug,
        }
    }

    /// Drain the global-hotkey channel, applying every queued action, and
    /// return the last one for logging. Never blocks.
    pub fn poll_hotkeys(&mut self) -> Option<HotkeyAction> {
        let mut last = None;
        while let Some(action) = self.hotkeys.try_recv() {
            self.apply_hotkey(action);
            last = Some(action);
        }
        last
    }

    /// Apply a hotkey (or wheel) action: blacklist gate, limit beep and the
    /// step-size mapping — matching the pre-Tauri host behaviour.
    pub fn apply_hotkey(&mut self, action: HotkeyAction) {
        use HotkeyAction as H;
        if matches!(action, H::OpenMenu | H::OpenMixer) {
            let step = self.config.volume_step as i16;
            let step_large = self.config.volume_step_large as i16;
            self.handle_action(hotkey_to_action(action, step, step_large));
            return;
        }
        // Blacklist gate: suppress hotkeys while a blacklisted app is focused.
        if let Some(proc) = foreground_process() {
            if crate::config::is_blacklisted(&self.config.blacklist, &proc) {
                log::debug!("hotkey blocked by blacklist ({proc})");
                beep_blocked(&self.config);
                return;
            }
        }
        log::debug!(
            "hotkey: {action:?} (current {}%)",
            self.last_state.percent()
        );
        let step = self.config.volume_step as i16;
        let step_large = self.config.volume_step_large as i16;
        self.handle_action(hotkey_to_action(action, step, step_large));
    }

    /// Central handler for every [`AppAction`] — the SSOT mutation point.
    /// Audio/config mutations are applied here and confirmed to the host via
    /// the sink; surface show/hide/toggle are routed to the host's window
    /// manager; the tray menu and overlay text go through the sink, while the
    /// settings-window internals stay logged stubs (the Settings webview
    /// drives settings through the command layer).
    pub fn handle_action(&mut self, action: AppAction) {
        use AppAction as A;
        use SurfaceId as S;

        match action {
            A::SetVolumePercent { percent } => {
                let pct = (percent.min(100) as f32) / 100.0;
                if let Err(e) = self.audio.set_volume(pct) {
                    log::warn!("{e}");
                } else if pct > 0.0 {
                    // A positive direct volume write is an explicit request
                    // to resume audible output after a mute fallback.
                    let _ = self.audio.set_mute(false);
                    self.mute_restore_volume = None;
                }
                self.publish_confirmed_state(true);
            }
            A::AdjustVolume { delta_percent } => {
                // Step actions use the cached audio-truth as the base so the
                // limit beep compares against the same reference as before
                // the mutation.
                let old = self.last_state;
                let target = crate::core::step_volume(old.volume, delta_percent as f32);
                log::debug!(
                    "action: adjust {delta_percent}% ({}% -> {:.0}%)",
                    old.percent(),
                    target * 100.0
                );
                if let Err(e) = self.audio.set_volume(target) {
                    log::warn!("{e}");
                } else if target > 0.0 {
                    let _ = self.audio.set_mute(false);
                    self.mute_restore_volume = None;
                }
                if target == old.volume && (old.volume == 0.0 || old.volume == 1.0) {
                    beep_limit(&self.config);
                }
                self.publish_confirmed_state(true);
            }
            A::ToggleMute => {
                if let Err(e) = self.toggle_mute() {
                    log::warn!("{e}");
                }
            }
            A::SetMute { muted } => {
                if let Err(e) = self.set_mute(muted) {
                    log::warn!("{e}");
                }
            }
            A::ResetVolume => {
                if let Err(e) = self.audio.set_volume(0.5) {
                    log::warn!("{e}");
                } else {
                    let _ = self.audio.set_mute(false);
                    self.mute_restore_volume = None;
                }
                self.publish_confirmed_state(true);
            }
            // ── Surfaces: routed to the host window manager. The Mixer is
            //    toggled by its hotkey (legacy app.rs behavior); Show/Hide
            //    keep explicit semantics. Settings/Help open on toggle (their
            //    current behavior). ──────────────────────────────────────────
            A::ShowSurface(S::Mixer) => {
                self.sink.open_surface("window-mixer");
            }
            A::ToggleSurface(S::Mixer) => {
                self.sink.toggle_surface("window-mixer");
            }
            A::HideSurface(S::Mixer) => {
                self.sink.close_surface("window-mixer");
            }
            A::ShowSurface(S::Settings) | A::ToggleSurface(S::Settings) => {
                self.sink.open_surface("window-settings");
            }
            A::HideSurface(S::Settings) => {
                self.sink.close_surface("window-settings");
            }
            A::ShowSurface(S::Help) | A::ToggleSurface(S::Help) => {
                self.sink.open_surface("window-help");
            }
            A::HideSurface(S::Help) => {
                self.sink.close_surface("window-help");
            }
            A::ShowSurface(S::Overlay)
            | A::HideSurface(S::Overlay)
            | A::ToggleSurface(S::Overlay) => {
                log::debug!(
                    "overlay surface show/hide intent is a no-op; the HUD comes from the sink"
                );
            }
            A::OpenTrayMenu | A::ShowSurface(S::Tray) | A::ToggleSurface(S::Tray) => {
                self.sink.show_tray_menu();
            }
            A::HideSurface(S::Tray) => {
                log::debug!("tray surface hide is a no-op (the tray is always resident)");
            }
            // ── Appearance: persist + adopt (keeps modifier re-registration
            //    and the confirmed-state publish in one place). ──────────
            A::SetTheme(theme) => {
                self.config.appearance.theme = theme;
                self.save_and_adopt();
            }
            A::SetMaterial(material) => {
                self.config.appearance.material = material;
                self.save_and_adopt();
            }
            A::SetMotion(motion) => {
                self.config.appearance.motion = motion;
                self.save_and_adopt();
            }
            A::SetAccent(accent) => {
                self.config.appearance.accent = accent;
                self.save_and_adopt();
            }
            A::SetLanguage(language) => {
                self.config.language = language;
                self.save_and_adopt();
            }
            A::ApplyConfigDraft(patch) => {
                if let Err(error) = self.update_settings(*patch) {
                    log::warn!("Settings draft rejected: {error}");
                } else if let Err(error) = self.save_config() {
                    log::warn!("Settings draft persist failed: {error}");
                }
            }
            A::ApplyConfig => {
                self.save_and_adopt();
            }
            A::CancelConfig => {
                self.force_reload_config();
            }
            A::ResetConfig => {
                self.config = crate::config::Config::default();
                self.save_and_adopt();
            }
            A::SetSessionVolume { id, percent } => {
                if let Err(e) = self.set_session_volume(&id, percent) {
                    log::warn!("Failed to set session volume for {}: {}", id, e);
                }
                self.publish_confirmed_state(false);
            }
            A::ToggleSessionMute { id } => {
                if let Err(e) = self.mute_session(&id) {
                    log::warn!("Failed to toggle session mute for {}: {}", id, e);
                }
                self.publish_confirmed_state(false);
            }
            A::OpenConfigLocation => {
                crate::config::open_in_editor();
                self.sink.overlay(
                    Some("Editing config — changes reload automatically".into()),
                    self.last_state,
                    self.config.clone(),
                );
            }
            A::ReloadConfig => {
                self.force_reload_config();
                self.sink.overlay(
                    Some("Config reloaded".into()),
                    self.last_state,
                    self.config.clone(),
                );
            }
            A::CheckUpdates => {
                // Blocking fetch (~seconds worst case). The Tauri tray
                // dispatch intercepts this command and runs the fetch off
                // the core lock; this arm serves the legacy host and tests.
                let message = crate::updates::check_for_updates(self.config.language);
                self.show_overlay_text(message);
            }
            A::AddBlacklistEntry(entry) => {
                let entry = crate::config::normalize_blacklist_entry(&entry);
                if !self.config.blacklist.iter().any(|e| e == &entry) {
                    self.config.blacklist.push(entry);
                }
                self.save_and_adopt();
            }
            A::RemoveBlacklistEntry(entry) => {
                let entry = crate::config::normalize_blacklist_entry(&entry);
                self.config.blacklist.retain(|e| e != &entry);
                self.save_and_adopt();
            }
            A::ClearBlacklist => {
                self.config.blacklist.clear();
                self.save_and_adopt();
            }
            A::ApplyRecommendedBlacklist => {
                crate::config::apply_recommended_blacklist(&mut self.config);
                self.save_and_adopt();
            }
            A::Exit => {
                log::info!("Exit requested — host teardown");
                self.sink.exit();
            }
        }
    }

    /// Restore a previously captured config after a failed persist attempt.
    ///
    /// Used by the shell when saving fails: the on-disk state may be absent or
    /// older, so the caller's last known-good snapshot is authoritative.
    pub fn restore_config(&mut self, config: crate::config::Config) {
        self.adopt_saved_config(config);
    }

    /// Persist the current config and adopt the normalized result (re-register
    /// hotkeys when the modifier changed, refresh status, publish state).
    pub fn save_config(&mut self) -> Result<(), String> {
        let saved = crate::config::save_validated(&self.config).map_err(|e| e.to_string())?;
        self.adopt_saved_config(saved);
        Ok(())
    }

    /// Validate and apply a settings draft, then persist it.
    ///
    /// Unlike `ApplyConfigDraft`, a persistence failure is returned so callers
    /// keep the dirty draft instead of presenting unsaved edits as saved.
    pub fn apply_settings_draft(
        &mut self,
        patch: crate::host_core::SettingsPatch,
    ) -> Result<(), String> {
        self.update_settings(patch)?;
        self.save_config()
    }

    /// Set the master volume for CLI callers, propagating backend errors.
    pub fn set_volume_percent(&mut self, percent: u8) -> Result<(), String> {
        let target = f32::from(percent.min(100)) / 100.0;
        self.audio.set_volume(target).map_err(|e| e.to_string())?;
        self.publish_confirmed_state(true);
        Ok(())
    }

    /// Set the master mute state for CLI callers, propagating backend errors.
    pub fn set_muted(&mut self, muted: bool) -> Result<(), String> {
        self.audio.set_mute(muted).map_err(|e| e.to_string())?;
        self.publish_confirmed_state(true);
        Ok(())
    }

    /// Toggle the master mute state for CLI callers, propagating backend errors.
    pub fn toggle_muted(&mut self) -> Result<(), String> {
        self.audio.toggle_mute().map_err(|e| e.to_string())?;
        self.publish_confirmed_state(true);
        Ok(())
    }

    /// Show a short text card on the overlay (host-facing seam used by the
    /// tray update check and other asynchronous results).
    pub fn show_overlay_text(&mut self, text: String) {
        self.sink
            .overlay(Some(text), self.last_state, self.config.clone());
    }

    /// Apply a partial settings patch to the in-memory config. Step sizes,
    /// overlay duration, beep ranges and colour thresholds are validated
    /// against the config.rs rules BEFORE mutating, so a rejected patch never
    /// leaves the in-memory config diverging from what `save_validated`
    /// accepts; appearance strings must be exact enum variant names. The
    /// blacklist patch is a full-list replace (draft semantics). Persistence
    /// is deliberately NOT part of this method so tests can exercise the
    /// mutation without touching the user's config file — the command layer
    /// calls [`AppCore::save_config`] afterwards.
    pub fn update_settings(&mut self, patch: SettingsPatch) -> Result<(), String> {
        // Parse appearance fields first so a bad enum string fails before any
        // step mutation (same fail-early behavior as before).
        let theme = patch.theme.as_deref().map(parse_theme).transpose()?;
        let material = patch.material.as_deref().map(parse_material).transpose()?;
        let motion = patch.motion.as_deref().map(parse_motion).transpose()?;
        let accent = patch.accent.as_deref().map(parse_accent).transpose()?;
        let language = patch.language.as_deref().map(parse_language).transpose()?;
        let app_label_style = patch
            .app_label_style
            .as_deref()
            .map(parse_app_label_style)
            .transpose()?;

        if let Some(hotkeys) = &patch.hotkeys {
            crate::config::validate_hotkeys(hotkeys).map_err(|e| e.to_string())?;
        }

        // Step sizes: validate the prospective values (patch values fall back
        // to the current config) against the same rules `save_validated`
        // enforces — identical error strings via config::validate_steps.
        let step = patch.volume_step.unwrap_or(self.config.volume_step);
        let large = patch
            .volume_step_large
            .unwrap_or(self.config.volume_step_large);
        crate::config::validate_steps(step, large).map_err(|e| e.to_string())?;

        // Overlay duration: 200..=10_000, identical string to config::validate.
        let overlay = patch
            .overlay_duration_ms
            .unwrap_or(self.config.overlay_duration_ms);
        if !(200..=10_000).contains(&overlay) {
            return Err("overlay_duration_ms: must be between 200 and 10000".into());
        }

        // Beep fields: prospective values validated against the same ranges
        // and strings as config::validate (37..=32_767 Hz, 10..=2_000 ms).
        if let Some(beep) = &patch.beep {
            let blocked_freq = beep.blocked_freq.unwrap_or(self.config.beep.blocked_freq);
            if !(37..=32_767).contains(&blocked_freq) {
                return Err("beep.blocked_freq: must be between 37 and 32767".into());
            }
            let blocked_ms = beep
                .blocked_duration_ms
                .unwrap_or(self.config.beep.blocked_duration_ms);
            if !(10..=2_000).contains(&blocked_ms) {
                return Err("beep.blocked_duration_ms: must be between 10 and 2000".into());
            }
            let limit_freq = beep.limit_freq.unwrap_or(self.config.beep.limit_freq);
            if !(37..=32_767).contains(&limit_freq) {
                return Err("beep.limit_freq: must be between 37 and 32767".into());
            }
            let limit_ms = beep
                .limit_duration_ms
                .unwrap_or(self.config.beep.limit_duration_ms);
            if !(10..=2_000).contains(&limit_ms) {
                return Err("beep.limit_duration_ms: must be between 10 and 2000".into());
            }
        }

        if let Some(size) = patch.ui_font_size {
            if !(9..=24).contains(&size) {
                return Err("ui_font_size: must be between 9 and 24".into());
            }
        }

        // Colour thresholds: prospective values through the shared
        // config::validate_thresholds (0..=100 + monotonic band order).
        if let Some(thresholds) = &patch.color_thresholds {
            let green = thresholds
                .green_up_to
                .unwrap_or(self.config.color_thresholds.green_up_to);
            let blue = thresholds
                .blue_up_to
                .unwrap_or(self.config.color_thresholds.blue_up_to);
            let orange = thresholds
                .orange_up_to
                .unwrap_or(self.config.color_thresholds.orange_up_to);
            crate::config::validate_thresholds(green, blue, orange).map_err(|e| e.to_string())?;
        }

        // Mutate — only after every prospective value passed.
        if let Some(step) = patch.volume_step {
            self.config.volume_step = step;
        }
        if let Some(large) = patch.volume_step_large {
            self.config.volume_step_large = large;
        }
        if let Some(theme) = theme {
            self.config.appearance.theme = theme;
        }
        if let Some(material) = material {
            self.config.appearance.material = material;
        }
        if let Some(motion) = motion {
            self.config.appearance.motion = motion;
        }
        if let Some(accent) = accent {
            self.config.appearance.accent = accent;
        }
        if let Some(overlay) = patch.overlay_duration_ms {
            self.config.overlay_duration_ms = overlay;
        }
        if let Some(hotkeys) = patch.hotkeys {
            self.config.hotkeys = Some(hotkeys);
        }
        if let Some(beep) = patch.beep {
            if let Some(v) = beep.enabled {
                self.config.beep.enabled = v;
            }
            if let Some(v) = beep.blocked_freq {
                self.config.beep.blocked_freq = v;
            }
            if let Some(v) = beep.blocked_duration_ms {
                self.config.beep.blocked_duration_ms = v;
            }
            if let Some(v) = beep.limit_freq {
                self.config.beep.limit_freq = v;
            }
            if let Some(v) = beep.limit_duration_ms {
                self.config.beep.limit_duration_ms = v;
            }
        }
        if let Some(thresholds) = patch.color_thresholds {
            if let Some(v) = thresholds.green_up_to {
                self.config.color_thresholds.green_up_to = v;
            }
            if let Some(v) = thresholds.blue_up_to {
                self.config.color_thresholds.blue_up_to = v;
            }
            if let Some(v) = thresholds.orange_up_to {
                self.config.color_thresholds.orange_up_to = v;
            }
        }
        if let Some(list) = patch.blacklist {
            self.config.blacklist = list
                .iter()
                .map(|s| crate::config::normalize_blacklist_entry(s))
                .collect();
        }
        if let Some(autostart) = patch.autostart {
            self.config.autostart = autostart;
        }
        if let Some(size) = patch.ui_font_size {
            self.config.ui_font_size = size;
        }
        if let Some(lang) = language {
            self.config.language = lang;
        }
        if let Some(style) = app_label_style {
            self.config.app_label_style = style;
        }
        Ok(())
    }

    /// The recommended blacklist presets for the current modifier (read-only;
    /// feeds the Settings Blacklist editor's "Apply Recommended" draft merge).
    pub fn recommended_blacklist(&self) -> Vec<String> {
        crate::config::recommended_blacklist(self.config.modifier)
    }

    /// Change the hotkey modifier: re-register every combo, persist the
    /// config, push the fresh status and confirmed state to the host.
    pub fn set_modifier(&mut self, modifier: HotkeyModifier) -> Result<(), String> {
        // Persist first so a failed save leaves no running side-effects
        // (legacy saved before adopting). `save_validated` returns the
        // normalized config; adopt it so memory matches disk.
        let mut next = self.config.clone();
        next.modifier = modifier;
        next.hotkeys = Some(HotkeyBindings::for_modifier(modifier));
        let saved = crate::config::save_validated(&next).map_err(|e| e.to_string())?;
        let bindings_changed = saved.hotkeys != self.config.hotkeys;
        let modifier_changed = saved.modifier != self.config.modifier;
        self.config = saved;
        // Resync the mtime so the 150 ms reloader never sees our own write as
        // an external change (same class as `adopt_saved_config`).
        self.last_config_mtime = config_mtime();
        if modifier_changed || bindings_changed {
            log::info!("config: global shortcuts changed — updating listener");
            self.hotkeys.set_bindings(&config_hotkeys(&self.config));
            // Keep the wheel-bridge modifier in sync on Windows (legacy synced
            // the wheel in every modifier-change path).
            #[cfg(target_os = "windows")]
            crate::wheel_win32::set_modifier(self.config.modifier);
            self.hotkey_status = self.hotkeys.status();
            self.sink.hotkeys(&self.hotkey_status);
        }
        self.publish_confirmed_state(false);
        Ok(())
    }

    /// The per-app session list. Windows enumerates WASAPI sessions of the
    /// default render device; other platforms return an empty list.
    pub fn sessions(&mut self) -> Vec<AudioSessionInfo> {
        self.sessions_source.list()
    }

    /// Set one session's volume. A stale/missing session id returns `Err` and
    /// the fresh session list is re-emitted so the frontend drops the dead row
    /// (spec §9.6). Unsupported platforms keep the no-op `Ok` contract.
    pub fn set_session_volume(&mut self, id: &str, pct: u8) -> Result<(), String> {
        self.sessions_source.set_volume(id, pct)
    }

    /// Toggle one session's mute. Same stale-id contract as
    /// [`Self::set_session_volume`].
    pub fn mute_session(&mut self, id: &str) -> Result<(), String> {
        self.sessions_source.mute(id)
    }

    /// Toggle the default output mute state and publish the exact state
    /// returned by the backend. The Tauri command uses this fallible seam so
    /// an unavailable endpoint is reported to the caller instead of silently
    /// leaving the UI on a stale mute label.
    pub fn toggle_mute(&mut self) -> Result<(), String> {
        let current = self.audio.get_state().map_err(|error| error.to_string())?;
        if current.muted {
            let restore = self.mute_restore_volume.take().unwrap_or(current.volume);
            self.audio
                .set_mute(false)
                .map_err(|error| error.to_string())?;
            self.audio
                .set_volume(restore)
                .map_err(|error| error.to_string())?;
        } else {
            self.mute_restore_volume = Some(current.volume);
            self.audio
                .set_mute(true)
                .map_err(|error| error.to_string())?;
            if let Err(error) = self.audio.set_volume(0.0) {
                let _ = self.audio.set_mute(false);
                return Err(error.to_string());
            }
        }
        let state = self.audio.get_state().map_err(|error| error.to_string())?;
        self.publish_state(state, true);
        Ok(())
    }

    /// Apply an explicit mute value using the same audible-zero fallback as
    /// [`Self::toggle_mute`].
    pub fn set_mute(&mut self, muted: bool) -> Result<(), String> {
        let current = self.audio.get_state().map_err(|error| error.to_string())?;
        if muted == current.muted && (!muted || current.volume == 0.0) {
            self.publish_state(current, true);
            return Ok(());
        }
        if muted {
            self.mute_restore_volume = Some(current.volume);
            self.audio
                .set_mute(true)
                .map_err(|error| error.to_string())?;
            self.audio
                .set_volume(0.0)
                .map_err(|error| error.to_string())?;
        } else {
            let restore = self.mute_restore_volume.take().unwrap_or(current.volume);
            self.audio
                .set_mute(false)
                .map_err(|error| error.to_string())?;
            self.audio
                .set_volume(restore)
                .map_err(|error| error.to_string())?;
        }
        let state = self.audio.get_state().map_err(|error| error.to_string())?;
        self.publish_state(state, true);
        Ok(())
    }

    /// Re-read the audio state and push the confirmed volume/mute to the
    /// host. `show_overlay` controls whether the native HUD overlay is shown:
    /// volume-mutating actions pass `true` (mirroring the legacy host's
    /// `publish_confirmed_state(ctx, show_overlay)`, which showed the HUD on
    /// every volume action); config-only paths pass `false`. The native
    /// overlay/tray renderers are host concerns and are driven through the
    /// sink's `overlay`/`volume` notifications.
    pub fn publish_confirmed_state(&mut self, show_overlay: bool) {
        let probe = self.audio.get_state();
        if !self.note_backend_result(&probe) {
            return;
        }
        let st = probe.expect("note_backend_result accepted Ok");
        self.publish_state(st, show_overlay);
    }

    fn publish_state(&mut self, st: VolumeState, show_overlay: bool) {
        self.last_state = st;
        log::debug!("publish: state={}%% muted={}", st.percent(), st.muted);
        self.sink.volume(st.percent(), st.muted);
        if show_overlay {
            self.sink.overlay(None, st, self.config.clone());
        }
    }

    /// Periodic external audio-state sync (media keys, other apps changing
    /// the volume OUTSIDE this app). Publishes when the confirmed state
    /// changed so the tray tooltip and open webviews stay fresh. The overlay
    /// is NOT re-shown for external changes — the native media-key flyout
    /// stays authoritative (mirrors the legacy 150 ms host timer, which only
    /// re-showed the HUD when the config had just reloaded).
    pub fn sync_external_state(&mut self) {
        let probe = self.audio.get_state();
        if !self.note_backend_result(&probe) {
            return;
        }
        let st = probe.expect("note_backend_result accepted Ok");
        if st != self.last_state {
            log::debug!("ext change: {}% muted={}", st.percent(), st.muted);
            self.last_state = st;
            self.sink.volume(st.percent(), st.muted);
        }
    }

    /// Track backend health across probes and emit the Ready ↔ Degraded
    /// transition exactly once per change. Returns true when the probe
    /// succeeded (caller continues publishing).
    fn note_backend_result(
        &mut self,
        probe: &Result<VolumeState, crate::audio::AudioError>,
    ) -> bool {
        match probe {
            Ok(_) => {
                if self.backend_status != BackendStatus::Ready {
                    self.backend_status = BackendStatus::Ready;
                    self.sink.backend(&self.backend_status);
                }
                true
            }
            Err(error) => {
                let status = BackendStatus::Degraded {
                    error: error.to_string(),
                };
                if self.backend_status != status {
                    self.backend_status = status;
                    self.sink.backend(&self.backend_status);
                }
                false
            }
        }
    }

    /// Snapshot getters for the host renderers (native overlay/tray).
    pub fn last_state(&self) -> VolumeState {
        self.last_state
    }

    /// A clone of the running config for host renderers.
    pub fn config(&self) -> Config {
        self.config.clone()
    }

    /// Reload the config when the file changed on disk. Re-registers hotkeys
    /// (and the wheel modifier on Windows) when the modifier changed, pushes
    /// the fresh state and shows the volume HUD. Returns true when a reload
    /// happened.
    pub fn reload_config_if_changed(&mut self) -> bool {
        let mtime = config_mtime();
        if mtime == self.last_config_mtime {
            return false;
        }
        let (new_cfg, config_notice) = match crate::config::load_existing_with_notice() {
            Ok(result) => result,
            Err(error) => {
                // Keep the last known-good in-memory config when an editor
                // briefly exposes a partial or malformed INI. Recording the
                // observed mtime prevents a tight retry loop; a later edit
                // produces a new mtime and is tried normally.
                self.last_config_mtime = mtime;
                self.config_notice = Some(crate::config::ConfigLoadNotice::InvalidExternalEdit);
                log::warn!("config reload ignored invalid INI: {error}");
                return false;
            }
        };
        self.last_config_mtime = mtime;
        self.config_notice = config_notice;
        let modifier_changed = new_cfg.modifier != self.config.modifier;
        let bindings_changed = new_cfg.hotkeys != self.config.hotkeys;
        log::info!(
            "config reloaded (step={}, step_large={}, overlay_ms={}, modifier={:?})",
            new_cfg.volume_step,
            new_cfg.volume_step_large,
            new_cfg.overlay_duration_ms,
            new_cfg.modifier
        );
        self.config = new_cfg;
        if modifier_changed || bindings_changed {
            self.hotkeys.set_bindings(&config_hotkeys(&self.config));
            #[cfg(target_os = "windows")]
            crate::wheel_win32::set_modifier(self.config.modifier);
            self.hotkey_status = self.hotkeys.status();
            self.sink.hotkeys(&self.hotkey_status);
        }
        self.publish_confirmed_state(true);
        true
    }

    /// Force a reload from disk (tray "Reload config" command).
    pub fn force_reload_config(&mut self) {
        self.last_config_mtime = None;
        self.reload_config_if_changed();
    }

    fn adopt_saved_config(&mut self, saved: Config) {
        let modifier_changed = saved.modifier != self.config.modifier;
        let bindings_changed = saved.hotkeys != self.config.hotkeys;
        self.config = saved;
        // Resync the mtime so a save never triggers the 150 ms reloader into
        // a spurious reload + HUD flash (legacy resynced here too).
        self.last_config_mtime = config_mtime();
        if modifier_changed || bindings_changed {
            log::info!("config: global shortcuts changed — updating listener");
            self.hotkeys.set_bindings(&config_hotkeys(&self.config));
            #[cfg(target_os = "windows")]
            crate::wheel_win32::set_modifier(self.config.modifier);
            self.hotkey_status = self.hotkeys.status();
            self.sink.hotkeys(&self.hotkey_status);
        }
        self.publish_confirmed_state(false);
    }

    fn save_and_adopt(&mut self) {
        match crate::config::save_validated(&self.config) {
            Ok(saved) => self.adopt_saved_config(saved),
            Err(e) => log::warn!("config persist failed: {e}"),
        }
    }

    fn appearance_payload(&self) -> AppearancePayload {
        let theme_resolved = resolved_theme_str(self.config.appearance.theme);
        AppearancePayload {
            theme_resolved: theme_resolved.to_string(),
            material: material_str(self.config.appearance.material).to_string(),
            motion: motion_str(self.config.appearance.motion).to_string(),
            accent: accent_str(self.config.appearance.accent).to_string(),
        }
    }
}

/// Resolve a configured theme mode to the concrete `"dark"`/`"light"`
/// string, matching the mixer surface's resolution exactly (System folds
/// the platform probe; unknown probes fall back to light).
pub fn resolved_theme_str(theme: ThemeMode) -> &'static str {
    match theme {
        ThemeMode::Dark => "dark",
        ThemeMode::Light => "light",
        ThemeMode::System => {
            if system_is_dark().unwrap_or(false) {
                "dark"
            } else {
                "light"
            }
        }
    }
}

fn material_str(m: MaterialMode) -> &'static str {
    match m {
        MaterialMode::Auto => "Auto",
        MaterialMode::Translucent => "Translucent",
        MaterialMode::Acrylic => "Acrylic",
        MaterialMode::Blur => "Blur",
        MaterialMode::Mica => "Mica",
        MaterialMode::Transparent => "Transparent",
        MaterialMode::LiquidGlass => "LiquidGlass",
        MaterialMode::Opaque => "Opaque",
    }
}

fn parse_material(s: &str) -> Result<MaterialMode, String> {
    match s {
        "Auto" => Ok(MaterialMode::Auto),
        "Translucent" => Ok(MaterialMode::Translucent),
        "Acrylic" => Ok(MaterialMode::Acrylic),
        "Blur" => Ok(MaterialMode::Blur),
        "Mica" => Ok(MaterialMode::Mica),
        "Transparent" => Ok(MaterialMode::Transparent),
        "LiquidGlass" => Ok(MaterialMode::LiquidGlass),
        "Opaque" => Ok(MaterialMode::Opaque),
        _ => Err(format!(
            "unknown material {s:?} (expected Auto | Translucent | Acrylic | Blur | Mica | Transparent | LiquidGlass | Opaque)"
        )),
    }
}

fn parse_theme(s: &str) -> Result<ThemeMode, String> {
    match s {
        "System" => Ok(ThemeMode::System),
        "Light" => Ok(ThemeMode::Light),
        "Dark" => Ok(ThemeMode::Dark),
        _ => Err(format!(
            "unknown theme {s:?} (expected System | Light | Dark)"
        )),
    }
}

fn parse_motion(s: &str) -> Result<MotionMode, String> {
    match s {
        "Full" => Ok(MotionMode::Full),
        "Reduced" => Ok(MotionMode::Reduced),
        "Disabled" => Ok(MotionMode::Disabled),
        _ => Err(format!(
            "unknown motion {s:?} (expected Full | Reduced | Disabled)"
        )),
    }
}

fn parse_accent(s: &str) -> Result<AccentMode, String> {
    match s {
        "System" => Ok(AccentMode::System),
        "Blue" => Ok(AccentMode::Blue),
        "Green" => Ok(AccentMode::Green),
        "Purple" => Ok(AccentMode::Purple),
        "Orange" => Ok(AccentMode::Orange),
        _ => Err(format!(
            "unknown accent {s:?} (expected System | Blue | Green | Purple | Orange)"
        )),
    }
}

fn parse_language(s: &str) -> Result<crate::config::Language, String> {
    match s {
        "en" => Ok(crate::config::Language::En),
        "vi" => Ok(crate::config::Language::Vi),
        other => Err(format!("language: unsupported value {other:?}")),
    }
}

fn parse_app_label_style(s: &str) -> Result<crate::config::AppLabelStyle, String> {
    match s {
        "friendly" => Ok(crate::config::AppLabelStyle::Friendly),
        "friendly_and_process" => Ok(crate::config::AppLabelStyle::FriendlyAndProcess),
        "process" => Ok(crate::config::AppLabelStyle::Process),
        other => Err(format!("app_label_style: unsupported value {other:?}")),
    }
}

fn motion_str(m: MotionMode) -> &'static str {
    match m {
        MotionMode::Full => "Full",
        MotionMode::Reduced => "Reduced",
        MotionMode::Disabled => "Disabled",
    }
}

fn accent_str(a: AccentMode) -> &'static str {
    match a {
        AccentMode::System => "System",
        AccentMode::Blue => "Blue",
        AccentMode::Green => "Green",
        AccentMode::Purple => "Purple",
        AccentMode::Orange => "Orange",
    }
}

fn config_hotkeys(config: &Config) -> HotkeyBindings {
    config
        .hotkeys
        .clone()
        .unwrap_or_else(|| HotkeyBindings::for_modifier(config.modifier))
}

/// Resolve the platform system theme (true = dark). Windows reads the
/// Personalize registry value; other platforms return `None` until the host
/// integration task probes them.
#[cfg(target_os = "windows")]
fn system_is_dark() -> Option<bool> {
    crate::ui::primitives::system_theme()
}

#[cfg(not(target_os = "windows"))]
fn system_is_dark() -> Option<bool> {
    None
}

/// Map a hotkey action to the shared action contract, resolving the
/// configured step sizes. Deliberately pure: the blacklist gate is a host
/// concern applied only to hotkey/wheel origin.
pub(crate) fn hotkey_to_action(action: HotkeyAction, step: i16, step_large: i16) -> AppAction {
    use HotkeyAction as H;
    match action {
        H::VolumeUp => AppAction::AdjustVolume {
            delta_percent: step,
        },
        H::VolumeDown => AppAction::AdjustVolume {
            delta_percent: -step,
        },
        H::VolumeUpLarge => AppAction::AdjustVolume {
            delta_percent: step_large,
        },
        H::VolumeDownLarge => AppAction::AdjustVolume {
            delta_percent: -step_large,
        },
        H::ToggleMute => AppAction::ToggleMute,
        H::Reset50 => AppAction::ResetVolume,
        H::OpenMixer => AppAction::ToggleSurface(SurfaceId::Mixer),
        H::OpenMenu => AppAction::OpenTrayMenu,
    }
}

/// Full image path for a process. Returns `None` when the process is gone or
/// access is denied.
#[cfg(target_os = "windows")]
fn process_path(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == 0 {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// Lowercase base name (e.g. `code.exe`) of a process, normalized for the
/// blacklist.
#[cfg(target_os = "windows")]
fn process_base_name(pid: u32) -> Option<String> {
    let path = process_path(pid)?;
    let base = path.rsplit('\\').next().unwrap_or(&path);
    if base.is_empty() {
        None
    } else {
        Some(crate::config::normalize_blacklist_entry(base))
    }
}

/// Turn `chrome.exe` into `Chrome` for executables without version metadata.
pub fn friendly_process_name(process: &str) -> String {
    let trimmed = process.trim();
    let lower = trimmed.to_ascii_lowercase();
    let stem = [".exe", ".app"]
        .iter()
        .find_map(|suffix| {
            lower
                .ends_with(suffix)
                .then(|| &trimmed[..trimmed.len() - suffix.len()])
        })
        .unwrap_or(trimmed);
    let mut chars = stem.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => trimmed.to_string(),
    }
}

/// Read a version-resource string (`ProductName` / `FileDescription`) from an
/// executable. The first translation listed by the resource is preferred,
/// with the common US-English block as a fallback.
#[cfg(target_os = "windows")]
fn version_string(data: &[u8], key: &str) -> Option<String> {
    use std::ffi::c_void;
    use windows_sys::Win32::Storage::FileSystem::VerQueryValueW;

    let mut translation_ptr: *mut c_void = std::ptr::null_mut();
    let mut translation_len = 0u32;
    let translation_query: Vec<u16> = "\\VarFileInfo\\Translation\0".encode_utf16().collect();
    let mut language = 0x0409u16;
    let mut codepage = 0x04B0u16;
    if unsafe {
        VerQueryValueW(
            data.as_ptr() as *const c_void,
            translation_query.as_ptr(),
            &mut translation_ptr,
            &mut translation_len,
        )
    } != 0
        && translation_len >= 4
        && !translation_ptr.is_null()
    {
        let words = unsafe { std::slice::from_raw_parts(translation_ptr as *const u16, 2) };
        language = words[0];
        codepage = words[1];
    }

    for block in [
        format!("\\StringFileInfo\\{language:04X}{codepage:04X}\\{key}"),
        format!("\\StringFileInfo\\040904B0\\{key}"),
    ] {
        let block: Vec<u16> = block.encode_utf16().chain(std::iter::once(0)).collect();
        let mut value_ptr: *mut c_void = std::ptr::null_mut();
        let mut value_len = 0u32;
        if unsafe {
            VerQueryValueW(
                data.as_ptr() as *const c_void,
                block.as_ptr(),
                &mut value_ptr,
                &mut value_len,
            )
        } == 0
            || value_ptr.is_null()
        {
            continue;
        }
        let raw =
            unsafe { std::slice::from_raw_parts(value_ptr as *const u16, value_len as usize) };
        let value: String = String::from_utf16_lossy(raw)
            .trim_end_matches('\0')
            .trim()
            .to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

/// Human-friendly application name from an executable path. Returns `None`
/// when the version resource is missing or only contains a generic Windows
/// product label; callers can then fall back to the process name.
#[cfg(target_os = "windows")]
pub fn friendly_executable_name(path: &str) -> Option<String> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW};

    let wide: Vec<u16> = std::ffi::OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut ignored = 0u32;
    let size = unsafe { GetFileVersionInfoSizeW(wide.as_ptr(), &mut ignored) };
    if size == 0 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    if unsafe { GetFileVersionInfoW(wide.as_ptr(), 0, size, data.as_mut_ptr() as *mut c_void) } == 0
    {
        return None;
    }
    for key in ["ProductName", "FileDescription"] {
        if let Some(value) = version_string(&data, key) {
            let generic = value.contains("Operating System")
                || value.eq_ignore_ascii_case("Microsoft Windows")
                || value.eq_ignore_ascii_case("Application");
            if !generic {
                return Some(value);
            }
        }
    }
    None
}

/// Extract the shell icon for an executable as a PNG data URL. Uses the
/// 32-bit icon and drops GDI handles on every path; `None` on any failure so
/// the UI can fall back to a letter tile.
#[cfg(target_os = "windows")]
fn executable_icon_data_url(path: &str) -> Option<String> {
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon;

    unsafe {
        let wide: Vec<u16> = std::ffi::OsStr::new(path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut file_info: SHFILEINFOW = zeroed();
        let result = SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut file_info,
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if result == 0 || file_info.hIcon == 0 {
            return None;
        }
        let icon = file_info.hIcon;
        let png = icon_to_png(icon);
        DestroyIcon(icon);
        png
    }
}

/// Convert an HICON to a PNG data URL. Kept separate so cleanup of the icon
/// itself stays in one place.
#[cfg(target_os = "windows")]
unsafe fn icon_to_png(icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON) -> Option<String> {
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    let mut info: ICONINFO = zeroed();
    if GetIconInfo(icon, &mut info) == 0 {
        return None;
    }
    let result = (|| {
        if info.hbmColor == 0 {
            return None;
        }
        let mut bitmap: BITMAP = zeroed();
        if GetObjectW(
            info.hbmColor as HGDIOBJ,
            size_of::<BITMAP>() as i32,
            &mut bitmap as *mut BITMAP as *mut c_void,
        ) == 0
        {
            return None;
        }
        let width = bitmap.bmWidth;
        let height = bitmap.bmHeight;
        if width <= 0 || height <= 0 {
            return None;
        }
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        let mut bmi: BITMAPINFO = zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..zeroed()
        };
        let dc = CreateCompatibleDC(0);
        if dc == 0 {
            return None;
        }
        let lines = GetDIBits(
            dc,
            info.hbmColor,
            0,
            height as u32,
            pixels.as_mut_ptr() as *mut c_void,
            &mut bmi,
            DIB_RGB_COLORS,
        );
        DeleteDC(dc);
        if lines == 0 {
            return None;
        }
        // DIB is BGRA. Some legacy icons carry no alpha; use an opaque tile
        // rather than rendering an invisible image.
        if !pixels.chunks_exact(4).any(|pixel| pixel[3] != 0) {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel[3] = 255;
            }
        }
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }

        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&pixels).ok()?;
        drop(writer);
        use base64::Engine as _;
        Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(out)
        ))
    })();

    if info.hbmColor != 0 {
        DeleteObject(info.hbmColor as HGDIOBJ);
    }
    if info.hbmMask != 0 {
        DeleteObject(info.hbmMask as HGDIOBJ);
    }
    result
}

/// Enumerate user-facing processes with a visible, titled top-level window.
/// The list feeds the Settings blacklist app picker. Background services and
/// the current VolumeControl process are excluded.
#[cfg(target_os = "windows")]
pub fn running_apps() -> Vec<RunningApp> {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    };

    struct Scan {
        apps: Vec<RunningApp>,
        current_pid: u32,
    }

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let scan = &mut *(lparam as *mut Scan);
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let title_len = GetWindowTextLengthW(hwnd);
        if title_len <= 0 {
            return 1;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 || pid == scan.current_pid {
            return 1;
        }
        let Some(path) = process_path(pid) else {
            return 1;
        };
        let base = path.rsplit('\\').next().unwrap_or(&path);
        let process = crate::config::normalize_blacklist_entry(base);
        let mut title = vec![0u16; title_len as usize + 1];
        let copied = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
        if copied <= 0 {
            return 1;
        }
        let title = String::from_utf16_lossy(&title[..copied as usize])
            .trim()
            .to_string();
        let display_name =
            friendly_executable_name(&path).unwrap_or_else(|| friendly_process_name(&process));
        let icon = if scan.apps.iter().any(|app| app.process == process) {
            None
        } else {
            executable_icon_data_url(&path)
        };
        merge_running_app(&mut scan.apps, process, title, Some(display_name), icon);
        1
    }

    let mut scan = Scan {
        apps: Vec::new(),
        current_pid: std::process::id(),
    };
    unsafe {
        EnumWindows(Some(collect), &mut scan as *mut Scan as isize);
    }
    scan.apps.sort_by(|a, b| {
        a.process
            .to_lowercase()
            .cmp(&b.process.to_lowercase())
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
    scan.apps
}

/// Non-Windows builds do not enumerate foreground windows; the picker stays
/// available but reports no running applications.
#[cfg(not(target_os = "windows"))]
pub fn running_apps() -> Vec<RunningApp> {
    Vec::new()
}

/// Resolve blacklist entries to icons/titles even when the app is not
/// currently running. Installed executables are found through the per-user
/// and machine `App Paths` registry keys; unresolved entries keep `None` and
/// are rendered with the letter-tile fallback.
#[cfg(target_os = "windows")]
pub fn resolve_apps(processes: &[String]) -> Vec<RunningApp> {
    let running = running_apps();
    let mut result = Vec::new();
    for raw in processes {
        let process = crate::config::normalize_blacklist_entry(raw);
        if process.is_empty() {
            continue;
        }
        if let Some(app) = running.iter().find(|app| app.process == process) {
            result.push(app.clone());
            continue;
        }
        let path = installed_executable_path(&process);
        let icon = path.as_deref().and_then(executable_icon_data_url);
        let display_name = path
            .as_deref()
            .and_then(friendly_executable_name)
            .unwrap_or_else(|| friendly_process_name(&process));
        result.push(RunningApp {
            process,
            title: String::new(),
            display_name: Some(display_name),
            icon,
        });
    }
    result
}

#[cfg(not(target_os = "windows"))]
pub fn resolve_apps(processes: &[String]) -> Vec<RunningApp> {
    processes
        .iter()
        .map(|process| RunningApp {
            process: process.clone(),
            title: String::new(),
            display_name: Some(friendly_process_name(process)),
            icon: None,
        })
        .collect()
}

/// Query the App Paths registry key for an installed executable. Expands
/// `%VAR%` segments with the current process environment; a path that cannot
/// be expanded is returned as-is and icon extraction simply fails soft.
#[cfg(target_os = "windows")]
fn installed_executable_path(process: &str) -> Option<String> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
        KEY_READ, REG_EXPAND_SZ, REG_SZ,
    };

    unsafe fn read_value(hive: HKEY, subkey: &str) -> Option<String> {
        let wide: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
        let mut key: HKEY = 0;
        if RegOpenKeyExW(hive, wide.as_ptr(), 0, KEY_READ, &mut key) != ERROR_SUCCESS {
            return None;
        }
        let mut kind = 0u32;
        let mut len = 0u32;
        let probe = RegQueryValueExW(
            key,
            std::ptr::null(),
            std::ptr::null(),
            &mut kind,
            std::ptr::null_mut(),
            &mut len,
        );
        if probe != ERROR_SUCCESS || (kind != REG_SZ && kind != REG_EXPAND_SZ) || len < 2 {
            RegCloseKey(key);
            return None;
        }
        let mut buf = vec![0u8; len as usize + 2];
        let mut data_len = buf.len() as u32;
        let result = RegQueryValueExW(
            key,
            std::ptr::null(),
            std::ptr::null(),
            &mut kind,
            buf.as_mut_ptr(),
            &mut data_len,
        );
        RegCloseKey(key);
        if result != ERROR_SUCCESS {
            return None;
        }
        let words = (data_len as usize / 2).min(buf.len() / 2);
        let raw: Vec<u16> = buf[..words * 2]
            .chunks_exact(2)
            .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
            .take_while(|ch| *ch != 0)
            .collect();
        let mut value = String::from_utf16_lossy(&raw);
        // Expand the common ProgramFiles-style variables without a second
        // Win32 dependency. App Paths values are simple absolute paths.
        for (name, replacement) in [
            ("ProgramFiles", std::env::var("ProgramFiles").ok()),
            ("ProgramFiles(x86)", std::env::var("ProgramFiles(x86)").ok()),
            ("SystemRoot", std::env::var("SystemRoot").ok()),
        ] {
            if let Some(replacement) = replacement {
                value = value.replace(&format!("%{name}%"), &replacement);
            }
        }
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }

    let mut subkeys = vec![
        format!("Software\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{process}"),
        format!("Software\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{process}"),
    ];
    for subkey in subkeys.drain(..) {
        for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            if let Some(path) = unsafe { read_value(hive, &subkey) } {
                return Some(path);
            }
        }
    }
    None
}

/// Lowercase base name (e.g. `code.exe`) of the foreground window's process.
#[cfg(target_os = "windows")]
fn foreground_process() -> Option<String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == 0 {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        process_base_name(pid)
    }
}

/// Lowercase base name of the foreground window's process on macOS.
#[cfg(target_os = "macos")]
fn foreground_process() -> Option<String> {
    use std::process::Command;
    // Use AppleScript to get the frontmost application
    let output = Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get name of first application process whose frontmost is true",
        ])
        .output()
        .ok()?;

    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if name.is_empty() {
            None
        } else {
            // Normalize: lowercase and ensure .app suffix for consistency with blacklist
            Some(crate::config::normalize_blacklist_entry(
                &name.to_lowercase(),
            ))
        }
    } else {
        None
    }
}

/// Lowercase base name of the foreground window's process on Linux.
#[cfg(target_os = "linux")]
fn foreground_process() -> Option<String> {
    use std::process::Command;

    // Method 1: Try xdotool first (most reliable if available)
    if let Ok(output) = Command::new("xdotool")
        .args(["getactivewindow", "--pid"])
        .output()
    {
        if output.status.success() {
            let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(pid) = pid_str.parse::<u32>() {
                if let Ok(comm) = std::fs::read_to_string(format!("/proc/{}/comm", pid)) {
                    let name = comm.trim().to_lowercase();
                    log::debug!("foreground_process: xdotool found PID {} -> {}", pid, name);
                    return Some(crate::config::normalize_blacklist_entry(&name));
                }
            }
        }
    }

    // Method 2: Fallback to xprop + wmctrl
    let output = Command::new("xprop")
        .args(["-root", "_NET_ACTIVE_WINDOW"])
        .output()
        .ok()?;

    if !output.status.success() {
        log::debug!("foreground_process: xprop failed");
        return None;
    }

    let window_id = String::from_utf8_lossy(&output.stdout);
    let window_id = window_id.split('#').nth(1)?.trim();

    let wmctrl_output = Command::new("wmctrl").arg("-lp").output().ok()?;

    if wmctrl_output.status.success() {
        let lines = String::from_utf8_lossy(&wmctrl_output.stdout);
        for line in lines.lines() {
            if line.contains(window_id) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    if let Ok(pid) = parts[1].parse::<u32>() {
                        if let Ok(comm) = std::fs::read_to_string(format!("/proc/{}/comm", pid)) {
                            let name = comm.trim().to_lowercase();
                            log::debug!("foreground_process: wmctrl found PID {} -> {}", pid, name);
                            return Some(crate::config::normalize_blacklist_entry(&name));
                        }
                    }
                }
            }
        }
    }

    // Method 3: Direct X11 query via x11rb (no CLI dependencies needed)
    if let Some(pid) = get_window_pid_x11() {
        if let Ok(comm) = std::fs::read_to_string(format!("/proc/{}/comm", pid)) {
            let name = comm.trim().to_lowercase();
            log::debug!("foreground_process: x11rb found PID {} -> {}", pid, name);
            return Some(crate::config::normalize_blacklist_entry(&name));
        }
    }

    // All methods failed - return None (better than wrong answer)
    log::warn!("foreground_process: could not determine foreground process on Linux");
    None
}

/// Direct X11 `_NET_ACTIVE_WINDOW` → `_NET_WM_PID` lookup via x11rb.
#[cfg(target_os = "linux")]
fn get_window_pid_x11() -> Option<u32> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    // EWMH atoms (`_NET_*`) are not in `AtomEnum` (core atoms only); they must
    // be interned at runtime.
    let active_win_atom = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .ok()?
        .reply()
        .ok()?
        .atom;

    let active_win_prop = conn
        .get_property(false, root, active_win_atom, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?;

    if active_win_prop.value.len() < 4 {
        return None;
    }

    let active_window = u32::from_ne_bytes([
        active_win_prop.value[0],
        active_win_prop.value[1],
        active_win_prop.value[2],
        active_win_prop.value[3],
    ]);

    if active_window == 0 {
        return None;
    }

    let pid_atom = conn
        .intern_atom(false, b"_NET_WM_PID")
        .ok()?
        .reply()
        .ok()?
        .atom;

    let pid_prop = conn
        .get_property(false, active_window, pid_atom, AtomEnum::CARDINAL, 0, 1)
        .ok()?
        .reply()
        .ok()?;

    if pid_prop.value.len() < 4 {
        return None;
    }

    let pid = u32::from_ne_bytes([
        pid_prop.value[0],
        pid_prop.value[1],
        pid_prop.value[2],
        pid_prop.value[3],
    ]);

    Some(pid)
}

/// Audible feedback for a blacklist-blocked hotkey (Win32 `Beep`).
#[cfg(target_os = "windows")]
fn beep_blocked(cfg: &Config) {
    if cfg.beep.enabled {
        unsafe {
            windows_sys::Win32::System::Diagnostics::Debug::Beep(
                cfg.beep.blocked_freq,
                cfg.beep.blocked_duration_ms,
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn beep_blocked(_cfg: &Config) {}

/// Audible feedback when a volume step cannot move further (limit reached).
#[cfg(target_os = "windows")]
fn beep_limit(cfg: &Config) {
    if cfg.beep.enabled {
        unsafe {
            windows_sys::Win32::System::Diagnostics::Debug::Beep(
                cfg.beep.limit_freq,
                cfg.beep.limit_duration_ms,
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn beep_limit(_cfg: &Config) {}

/// Last-modified time of the config file (None if it doesn't exist yet).
pub fn config_mtime() -> Option<std::time::SystemTime> {
    std::fs::metadata(crate::config::config_path())
        .and_then(|m| m.modified())
        .ok()
}

/// Map a tray menu command to the shared action contract (no blacklist gate,
/// matching the pre-Tauri host). Every platform host (native Windows tray
/// and the macOS/Linux Tauri tray) dispatches through this mapping.
pub fn tray_command_to_action(cmd: crate::tray_common::TrayCommand) -> AppAction {
    use crate::tray_common::TrayCommand as C;
    use SurfaceId as S;
    match cmd {
        C::ToggleMute => AppAction::ToggleMute,
        C::Reset50 => AppAction::ResetVolume,
        C::OpenMixer => AppAction::ToggleSurface(S::Mixer),
        C::Help => AppAction::ShowSurface(S::Help),
        C::Settings => AppAction::ToggleSurface(S::Settings),
        C::EditConfig => AppAction::OpenConfigLocation,
        C::ReloadConfig => AppAction::ReloadConfig,
        C::CheckUpdates => AppAction::CheckUpdates,
        C::Exit => AppAction::Exit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::AppAction;

    #[test]
    fn hotkey_to_action_maps_all_actions_with_steps() {
        use crate::hotkeys::HotkeyAction as H;
        let step: i16 = 1;
        let large: i16 = 10;
        assert_eq!(
            hotkey_to_action(H::VolumeUp, step, large),
            AppAction::AdjustVolume { delta_percent: 1 }
        );
        assert_eq!(
            hotkey_to_action(H::VolumeDown, step, large),
            AppAction::AdjustVolume { delta_percent: -1 }
        );
        assert_eq!(
            hotkey_to_action(H::VolumeUpLarge, step, large),
            AppAction::AdjustVolume { delta_percent: 10 }
        );
        assert_eq!(
            hotkey_to_action(H::VolumeDownLarge, step, large),
            AppAction::AdjustVolume { delta_percent: -10 }
        );
        assert_eq!(
            hotkey_to_action(H::ToggleMute, step, large),
            AppAction::ToggleMute
        );
        assert_eq!(
            hotkey_to_action(H::Reset50, step, large),
            AppAction::ResetVolume
        );
        assert_eq!(
            hotkey_to_action(H::OpenMixer, step, large),
            AppAction::ToggleSurface(SurfaceId::Mixer)
        );
        assert_eq!(
            hotkey_to_action(H::OpenMenu, step, large),
            AppAction::OpenTrayMenu
        );
    }

    #[test]
    fn hotkey_to_action_respects_configured_step_sizes() {
        use crate::hotkeys::HotkeyAction as H;
        assert_eq!(
            hotkey_to_action(H::VolumeUp, 5, 25),
            AppAction::AdjustVolume { delta_percent: 5 }
        );
        assert_eq!(
            hotkey_to_action(H::VolumeUpLarge, 5, 25),
            AppAction::AdjustVolume { delta_percent: 25 }
        );
    }

    #[test]
    fn tray_command_to_action_maps_all_commands() {
        use crate::tray_common::TrayCommand as C;
        assert_eq!(tray_command_to_action(C::ToggleMute), AppAction::ToggleMute);
        assert_eq!(tray_command_to_action(C::Reset50), AppAction::ResetVolume);
        assert_eq!(
            tray_command_to_action(C::OpenMixer),
            AppAction::ToggleSurface(SurfaceId::Mixer)
        );
        assert_eq!(
            tray_command_to_action(C::Help),
            AppAction::ShowSurface(SurfaceId::Help)
        );
        assert_eq!(
            tray_command_to_action(C::Settings),
            AppAction::ToggleSurface(SurfaceId::Settings)
        );
        assert_eq!(
            tray_command_to_action(C::EditConfig),
            AppAction::OpenConfigLocation
        );
        assert_eq!(
            tray_command_to_action(C::CheckUpdates),
            AppAction::CheckUpdates
        );
        assert_eq!(
            tray_command_to_action(C::ReloadConfig),
            AppAction::ReloadConfig
        );
        assert_eq!(tray_command_to_action(C::Exit), AppAction::Exit);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn executable_icon_is_a_png_data_url() {
        let executable = std::env::current_exe().expect("current exe");
        let icon = executable_icon_data_url(&executable.to_string_lossy())
            .expect("shell icon for the test executable");
        assert!(icon.starts_with("data:image/png;base64,"));
        assert!(icon.len() > 100);
    }

    #[test]
    fn friendly_process_names_title_case_the_executable() {
        assert_eq!(friendly_process_name("chrome.exe"), "Chrome");
        assert_eq!(friendly_process_name("Code.EXE"), "Code");
        assert_eq!(friendly_process_name("firefox"), "Firefox");
        assert_eq!(friendly_process_name(""), "");
    }

    #[test]
    fn running_apps_merge_by_process_and_keep_first_title() {
        let mut apps = Vec::new();
        merge_running_app(
            &mut apps,
            "chrome.exe".into(),
            "New Tab".into(),
            Some("Google Chrome".into()),
            None,
        );
        merge_running_app(
            &mut apps,
            "chrome.exe".into(),
            "Other window".into(),
            Some("Chrome fallback".into()),
            None,
        );
        merge_running_app(
            &mut apps,
            "code.exe".into(),
            String::new(),
            Some("Visual Studio Code".into()),
            None,
        );
        merge_running_app(&mut apps, String::new(), "ignored".into(), None, None);
        assert_eq!(
            apps,
            vec![
                RunningApp {
                    process: "chrome.exe".into(),
                    title: "New Tab".into(),
                    display_name: Some("Google Chrome".into()),
                    icon: None,
                },
                RunningApp {
                    process: "code.exe".into(),
                    title: String::new(),
                    display_name: Some("Visual Studio Code".into()),
                    icon: None,
                },
            ]
        );
    }

    #[test]
    fn config_mtime_is_none_then_some_and_stable() {
        let _guard = crate::config::CONFIG_DIR_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Point the config path at a fresh temp dir (hermetic; never touches
        // the real user config on any platform).
        let tmp = std::env::temp_dir().join(format!("volumectl-mtime-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let old = std::env::var_os("VOLUMECTL_CONFIG_DIR");
        std::env::set_var("VOLUMECTL_CONFIG_DIR", &tmp);

        // Fresh: the config file does not exist yet.
        assert_eq!(config_mtime(), None, "fresh config dir has no mtime");

        // Write the config: mtime becomes Some and stays stable on re-reads.
        std::fs::write(crate::config::config_path(), b"{}").unwrap();
        let t1 = config_mtime().expect("mtime after write");
        assert_eq!(config_mtime(), Some(t1), "mtime stable without writes");

        // Changed: rewriting the file moves the mtime. The 30 ms sleep covers
        // coarse-granularity filesystems (NTFS is ~100 ns); ext4 uses 1 s so
        // the change assertion is Windows-gated to stay deterministic.
        #[cfg(target_os = "windows")]
        {
            std::thread::sleep(std::time::Duration::from_millis(30));
            std::fs::write(crate::config::config_path(), b"{ \"x\": 1 }").unwrap();
            let t2 = config_mtime().expect("mtime after rewrite");
            assert_ne!(config_mtime(), Some(t1), "rewrite moves the mtime");
            let _ = t2;
        }

        match old {
            Some(v) => std::env::set_var("VOLUMECTL_CONFIG_DIR", v),
            None => std::env::remove_var("VOLUMECTL_CONFIG_DIR"),
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
