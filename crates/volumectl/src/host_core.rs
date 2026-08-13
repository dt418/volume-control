//! Cross-platform application core — the single source of truth (SSOT).
//!
//! [`AppCore`] owns the config, the audio backend, the global hotkey
//! listener, and the last confirmed volume state, and applies every
//! [`AppAction`] emitted by any surface. It is deliberately free of native
//! surface plumbing (overlay/tray/webview windows): the host routes surface
//! open/close through the [`EventSink`], and every confirmed volume change is
//! pushed to the host as a `state://volume` event. The Tauri host wires the
//! sink in `src-tauri`, and the native HUD overlay/tray return in the host
//! integration task.

use std::sync::Arc;

use serde::Serialize;

use crate::audio::{AudioBackend, VolumeState};
use crate::config::{Config, HotkeyModifier};
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
}

/// One application audio session in the per-app mixer. The Windows WASAPI
/// source (Task 2b) enumerates these; other platforms report an empty list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioSessionInfo {
    pub id: String,
    pub name: String,
    pub pct: u8,
    pub muted: bool,
    pub active: bool,
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
    pub volume_pct: u8,
    pub muted: bool,
    pub hotkey_status: Vec<HotkeyRegResult>,
    pub appearance: AppearancePayload,
    pub sessions: Vec<AudioSessionInfo>,
    pub sessions_supported: bool,
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
    /// The host should open the surface with `label` (e.g. `"window-mixer"`).
    fn open_surface(&self, _label: &str) {}
    /// The host should close/destroy the surface with `label`.
    fn close_surface(&self, _label: &str) {}
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
    last_config_mtime: Option<std::time::SystemTime>,
}

impl AppCore {
    /// Create the core: register the global hotkeys for `modifier` and take
    /// the first confirmed audio snapshot.
    pub fn new(
        audio: Box<dyn AudioBackend>,
        config: Config,
        modifier: HotkeyModifier,
        sink: Arc<dyn EventSink>,
    ) -> Result<Self, String> {
        let hotkeys = GlobalHotkeys::new(modifier)?;
        let last_state = audio.get_state().unwrap_or(VolumeState {
            volume: 0.5,
            muted: false,
        });
        let hotkey_status = hotkeys.status();
        #[cfg(target_os = "windows")]
        let sessions_source: Box<dyn SessionsSource> =
            Box::new(crate::audio_sessions_win32::WindowsSessions);
        #[cfg(not(target_os = "windows"))]
        let sessions_source: Box<dyn SessionsSource> = Box::new(NoopSessions);
        Ok(Self {
            audio,
            hotkeys,
            config,
            last_state,
            hotkey_status,
            sink,
            sessions_source,
            last_config_mtime: config_mtime(),
        })
    }

    /// Snapshot for a webview mount: current config, confirmed state,
    /// hotkey status, resolved appearance and the session list.
    pub fn bootstrap(&mut self) -> BootstrapPayload {
        if let Ok(st) = self.audio.get_state() {
            self.last_state = st;
        }
        BootstrapPayload {
            config: self.config.clone(),
            volume_pct: self.last_state.percent(),
            muted: self.last_state.muted,
            hotkey_status: self.hotkey_status.clone(),
            appearance: self.appearance_payload(),
            sessions: self.sessions(),
            sessions_supported: self.sessions_source.supported(),
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
    /// manager; native-only intents (settings window internals, tray menu,
    /// overlay text) are logged stubs until the host integration task.
    pub fn handle_action(&mut self, action: AppAction) {
        use AppAction as A;
        use SurfaceId as S;

        match action {
            A::SetVolumePercent { percent } => {
                let pct = (percent.min(100) as f32) / 100.0;
                if let Err(e) = self.audio.set_volume(pct) {
                    log::warn!("{e}");
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
                }
                if target == old.volume && (old.volume == 0.0 || old.volume == 1.0) {
                    beep_limit(&self.config);
                }
                self.publish_confirmed_state(true);
            }
            A::ToggleMute => {
                if let Err(e) = self.audio.toggle_mute() {
                    log::warn!("{e}");
                }
                self.publish_confirmed_state(true);
            }
            A::SetMute { muted } => {
                if let Err(e) = self.audio.set_mute(muted) {
                    log::warn!("{e}");
                }
                self.publish_confirmed_state(true);
            }
            A::ResetVolume => {
                if let Err(e) = self.audio.set_volume(0.5) {
                    log::warn!("{e}");
                }
                self.publish_confirmed_state(true);
            }
            // ── Surfaces: routed to the host window manager. ─────────────
            A::ShowSurface(S::Mixer) | A::ToggleSurface(S::Mixer) => {
                self.sink.open_surface("window-mixer");
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
                log::debug!("overlay surface not wired in AppCore (host integration task)");
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
            // ── Not wired yet (host integration / webview surfaces). ─────
            A::ApplyConfig | A::CancelConfig | A::ResetConfig => {
                log::debug!(
                    "settings-window intents are driven by the webview surface (later task)"
                );
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
            A::AddBlacklistEntry(_)
            | A::RemoveBlacklistEntry(_)
            | A::ClearBlacklist
            | A::ApplyRecommendedBlacklist => {
                log::debug!("blacklist edits are wired via the Settings webview (later task)");
            }
            A::Exit => {
                log::info!("Exit requested — host teardown");
                self.sink.exit();
            }
        }
    }

    /// Persist the current config and adopt the normalized result (re-register
    /// hotkeys when the modifier changed, refresh status, publish state).
    pub fn save_config(&mut self) -> Result<(), String> {
        let saved = crate::config::save_validated(&self.config).map_err(|e| e.to_string())?;
        self.adopt_saved_config(saved);
        Ok(())
    }

    /// Apply a partial settings patch to the in-memory config. Step sizes are
    /// validated against the config.rs rules (1..=50, large > small) BEFORE
    /// mutating, so a rejected patch never leaves the in-memory config
    /// diverging from what `save_validated` accepts; appearance strings must
    /// be exact enum variant names. Persistence is deliberately NOT part of
    /// this method so tests can exercise the mutation without touching the
    /// user's config file — the command layer calls [`AppCore::save_config`]
    /// afterwards.
    pub fn update_settings(&mut self, patch: SettingsPatch) -> Result<(), String> {
        // Parse appearance fields first so a bad enum string fails before any
        // step mutation (same fail-early behavior as before).
        let theme = patch.theme.as_deref().map(parse_theme).transpose()?;
        let material = patch.material.as_deref().map(parse_material).transpose()?;
        let motion = patch.motion.as_deref().map(parse_motion).transpose()?;
        let accent = patch.accent.as_deref().map(parse_accent).transpose()?;

        // Step sizes: validate the prospective values (patch values fall back
        // to the current config) against the same rules `save_validated`
        // enforces — identical error strings via config::validate_steps.
        let step = patch.volume_step.unwrap_or(self.config.volume_step);
        let large = patch
            .volume_step_large
            .unwrap_or(self.config.volume_step_large);
        crate::config::validate_steps(step, large).map_err(|e| e.to_string())?;

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
        Ok(())
    }

    /// Change the hotkey modifier: re-register every combo, persist the
    /// config, push the fresh status and confirmed state to the host.
    pub fn set_modifier(&mut self, modifier: HotkeyModifier) -> Result<(), String> {
        self.config.modifier = modifier;
        self.hotkeys.set_modifier(modifier);
        // Keep the wheel-bridge modifier in sync on Windows (legacy synced
        // the wheel in every modifier-change path).
        #[cfg(target_os = "windows")]
        crate::wheel_win32::set_modifier(modifier);
        self.hotkey_status = self.hotkeys.status();
        self.sink.hotkeys(&self.hotkey_status);
        crate::config::save_validated(&self.config).map_err(|e| e.to_string())?;
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

    /// Re-read the audio state and push the confirmed volume/mute to the
    /// host. `show_overlay` controls whether the native HUD overlay is shown:
    /// volume-mutating actions pass `true` (mirroring the legacy host's
    /// `publish_confirmed_state(ctx, show_overlay)`, which showed the HUD on
    /// every volume action); config-only paths pass `false`. The native
    /// overlay/tray renderers are host concerns and are driven through the
    /// sink's `overlay`/`volume` notifications.
    pub fn publish_confirmed_state(&mut self, show_overlay: bool) {
        let Ok(st) = self.audio.get_state() else {
            return;
        };
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
        let Ok(st) = self.audio.get_state() else {
            return;
        };
        if st != self.last_state {
            log::debug!("ext change: {}% muted={}", st.percent(), st.muted);
            self.last_state = st;
            self.sink.volume(st.percent(), st.muted);
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

    /// Resolve the native overlay's adaptive appearance. Windows-only type;
    /// called by the Windows host (the caps snapshot it captured at startup).
    #[cfg(target_os = "windows")]
    pub fn overlay_appearance(
        &self,
        caps: &crate::ui::UiCapabilities,
    ) -> crate::overlay::OverlayAppearance {
        crate::overlay::OverlayAppearance::resolve(
            &self.config,
            caps,
            crate::ui::primitives::system_theme,
        )
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
        self.last_config_mtime = mtime;
        let new_cfg = crate::config::load();
        let modifier_changed = new_cfg.modifier != self.config.modifier;
        log::info!(
            "config reloaded (step={}, step_large={}, overlay_ms={}, modifier={:?})",
            new_cfg.volume_step,
            new_cfg.volume_step_large,
            new_cfg.overlay_duration_ms,
            new_cfg.modifier
        );
        self.config = new_cfg;
        if modifier_changed {
            self.hotkeys.set_modifier(self.config.modifier);
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
        self.config = saved;
        // Resync the mtime so a save never triggers the 150 ms reloader into
        // a spurious reload + HUD flash (legacy resynced here too).
        self.last_config_mtime = config_mtime();
        if modifier_changed {
            log::info!("config: modifier changed — updating global listener");
            self.hotkeys.set_modifier(self.config.modifier);
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
        let theme_resolved = match self.config.appearance.theme {
            ThemeMode::Dark => "dark",
            ThemeMode::Light => "light",
            ThemeMode::System => {
                // Matches the native renderer's contract: unknown system
                // theme falls back to the light palette.
                if system_is_dark().unwrap_or(false) {
                    "dark"
                } else {
                    "light"
                }
            }
        };
        AppearancePayload {
            theme_resolved: theme_resolved.to_string(),
            material: material_str(self.config.appearance.material).to_string(),
            motion: motion_str(self.config.appearance.motion).to_string(),
            accent: accent_str(self.config.appearance.accent).to_string(),
        }
    }
}

fn material_str(m: MaterialMode) -> &'static str {
    match m {
        MaterialMode::Auto => "Auto",
        MaterialMode::Translucent => "Translucent",
        MaterialMode::Opaque => "Opaque",
    }
}

fn parse_material(s: &str) -> Result<MaterialMode, String> {
    match s {
        "Auto" => Ok(MaterialMode::Auto),
        "Translucent" => Ok(MaterialMode::Translucent),
        "Opaque" => Ok(MaterialMode::Opaque),
        _ => Err(format!(
            "unknown material {s:?} (expected Auto | Translucent | Opaque)"
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

/// Lowercase base name (e.g. `code.exe`) of the foreground window's process.
#[cfg(target_os = "windows")]
fn foreground_process() -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
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
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let base = path.rsplit('\\').next().unwrap_or(&path);
        Some(base.to_lowercase())
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
/// matching the pre-Tauri host). Windows-only: the tray exists only there.
#[cfg(target_os = "windows")]
pub fn tray_command_to_action(cmd: crate::tray::TrayCommand) -> AppAction {
    use crate::tray::TrayCommand as C;
    use SurfaceId as S;
    match cmd {
        C::ToggleMute => AppAction::ToggleMute,
        C::Reset50 => AppAction::ResetVolume,
        C::OpenMixer => AppAction::ToggleSurface(S::Mixer),
        C::Help => AppAction::ShowSurface(S::Help),
        C::Settings => AppAction::ToggleSurface(S::Settings),
        C::EditConfig => AppAction::OpenConfigLocation,
        C::ReloadConfig => AppAction::ReloadConfig,
        C::Exit => AppAction::Exit,
    }
}
