//! eframe application: root Mixer viewport, immediate child viewports, and
//! the AppCore bridge loop.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{ViewportBuilder, ViewportCommand, ViewportId};

use volumectl::config::{Config, ConfigLoadNotice, HotkeyModifier, Language};
use volumectl::host_core::{AppCore, BackendStatus, RunningApp};
use volumectl::hotkeys::HotkeyRegResult;
use volumectl::ui::{AppAction, AppState, AudioSessionState, SurfaceId, UiStatus};

use crate::i18n::{tr, Key};
use crate::placement::{frame_for, logical_size, WorkArea};
use crate::settings::model::{FieldError, SettingsDraft, Tab};
use crate::settings::recorder::Recorder;
use crate::settings::update_card::UpdateCard;
use crate::settings::views::{self, PickerState, SettingsIntent};
use crate::single_instance::PrimaryGuard;
use crate::sink::EguiEvent;
use crate::style;
use crate::surfaces::help::{self, HelpIntent, HelpView};
use crate::surfaces::tray::{Tray, TrayAction};
use crate::vibrancy::{self, VibrancyState};

/// Cadence for hotkey/audio/config polling, matching the legacy host timer.
const SLOW_POLL: Duration = Duration::from_millis(50);

/// Off-screen position for the parked Mixer root.
///
/// eframe 0.36 runs no UI pass while the root viewport is hidden, so hiding it
/// would also stop the immediate child viewports. The Mixer root is parked
/// off-screen at 1×1 instead (the plan's documented fallback).
const PARKED_POSITION: egui::Pos2 = egui::pos2(-32_000.0, -32_000.0);

/// Startup options parsed from the command line.
#[derive(Debug, Clone, Copy)]
pub struct LaunchOptions {
    pub smoke: bool,
    pub smoke_ms: u64,
    pub show_all: bool,
    pub target_surface: Option<SurfaceId>,
    pub settings_tab: Option<Tab>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            smoke: false,
            smoke_ms: 2_500,
            show_all: false,
            target_surface: None,
            settings_tab: None,
        }
    }
}

/// Stable viewport id for a surface.
#[must_use]
pub fn viewport_id(surface: SurfaceId) -> ViewportId {
    match surface {
        SurfaceId::Overlay => ViewportId::from_hash_of("volumecontrol-overlay"),
        SurfaceId::Mixer => ViewportId::ROOT,
        SurfaceId::Settings => ViewportId::from_hash_of("volumecontrol-settings"),
        SurfaceId::Help => ViewportId::from_hash_of("volumecontrol-help"),
        SurfaceId::Tray => ViewportId::from_hash_of("volumecontrol-tray"),
    }
}

/// Viewport builder for one surface, positioned by the shared placement math.
#[must_use]
pub fn builder_for(
    surface: SurfaceId,
    scale: f32,
    area: &WorkArea,
    language: Language,
) -> ViewportBuilder {
    let frame = frame_for(surface, area, scale);
    let (logical_w, logical_h) = logical_size(surface);
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let mut builder = ViewportBuilder::default()
        .with_title(vibrancy::title_for(surface, language))
        .with_inner_size(egui::vec2(logical_w, logical_h))
        .with_position(egui::pos2(
            (frame.x as f32 / scale).round(),
            (frame.y as f32 / scale).round(),
        ));
    if let Some(icon) = crate::icon::app_icon() {
        builder = builder.with_icon(icon);
    }
    match surface {
        SurfaceId::Mixer => builder
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false),
        SurfaceId::Overlay => builder
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_always_on_top(),
        SurfaceId::Settings => builder
            .with_resizable(true)
            .with_transparent(true)
            .with_min_inner_size(egui::vec2(620.0, 520.0))
            .with_taskbar(true),
        SurfaceId::Help => builder
            .with_resizable(true)
            .with_transparent(true)
            .with_min_inner_size(egui::vec2(420.0, 360.0))
            .with_taskbar(true),
        SurfaceId::Tray => builder.with_visible(false),
    }
}

/// The eframe application.
pub struct VolumeControlApp {
    core: AppCore,
    events: Receiver<EguiEvent>,
    state: AppState,
    config: Config,
    /// Hotkey registration status; consumed by the Settings recorder tabs.
    #[allow(dead_code)]
    hotkey_status: Vec<HotkeyRegResult>,
    options: LaunchOptions,
    root_shown: bool,
    root_visible_last: bool,
    quitting: bool,
    vibrancy: VibrancyState,
    is_dark: bool,
    applied_dark: Option<bool>,
    applied_title_language: Option<Language>,
    last_poll: Instant,
    last_sessions_poll: Instant,
    sessions_supported: bool,
    mixer_query: String,
    overlay_text: Option<String>,
    overlay_volume: u8,
    overlay_muted: bool,
    overlay_until: Option<Instant>,
    parked: bool,
    settings_draft: SettingsDraft,
    settings_tab: Tab,
    settings_error: Option<FieldError>,
    recorder: Recorder,
    picker: PickerState,
    running_apps: Vec<RunningApp>,
    update_card: UpdateCard,
    update_rx: Option<Receiver<Result<volumectl::updates::UpdateInfo, String>>>,
    update_url: Option<String>,
    pending_modifier: Option<HotkeyModifier>,
    config_notice: Option<ConfigLoadNotice>,
    help_query: String,
    tray: Option<Tray>,
    tray_actions: Receiver<TrayAction>,
    primary: Option<PrimaryGuard>,
    tray_volume: u8,
    tray_language: Language,
}

impl VolumeControlApp {
    /// Build the app from a configured core and the matching sink receiver.
    #[must_use]
    pub fn new(mut core: AppCore, events: Receiver<EguiEvent>, options: LaunchOptions) -> Self {
        let payload = core.bootstrap();
        let mut state = AppState::from_audio(payload.volume_pct, payload.muted, None);
        state.theme = payload.config.appearance.theme;
        state.material = payload.config.appearance.material;
        state.motion = payload.config.appearance.motion;
        state.accent = payload.config.appearance.accent;
        state.language = payload.config.language;
        state.sessions = payload.sessions.iter().map(session_state).collect();
        if let BackendStatus::Degraded { error } = &payload.backend_status {
            state.status = UiStatus::Error(error.clone());
        }

        let target = options.target_surface;
        let mut root_shown = !matches!(target, Some(surface) if surface != SurfaceId::Mixer);
        if options.show_all {
            root_shown = true;
        }
        if let Some(surface) = target {
            state.show(surface);
        } else {
            state.show(SurfaceId::Mixer);
        }
        if options.show_all {
            for surface in [
                SurfaceId::Mixer,
                SurfaceId::Settings,
                SurfaceId::Help,
                SurfaceId::Overlay,
            ] {
                state.show(surface);
            }
        }
        if !root_shown {
            state.hide(SurfaceId::Mixer);
        }

        let is_dark = payload.appearance.theme_resolved == "dark";
        let config = payload.config.clone();
        let settings_draft = SettingsDraft::from_config(&config);
        let (tray_sender, tray_actions) = mpsc::channel();
        let tray = Tray::new(config.language, payload.volume_pct, tray_sender).ok();
        let tray_volume = payload.volume_pct;
        let tray_language = config.language;
        Self {
            core,
            events,
            state,
            config,
            hotkey_status: payload.hotkey_status,
            options,
            root_shown,
            root_visible_last: true,
            quitting: false,
            vibrancy: VibrancyState::new(),
            is_dark,
            applied_dark: None,
            applied_title_language: None,
            last_poll: Instant::now(),
            last_sessions_poll: Instant::now(),
            sessions_supported: payload.sessions_supported,
            mixer_query: String::new(),
            overlay_text: None,
            overlay_volume: payload.volume_pct,
            overlay_muted: payload.muted,
            overlay_until: None,
            parked: false,
            settings_draft,
            settings_tab: options.settings_tab.unwrap_or_default(),
            settings_error: None,
            recorder: Recorder::new(),
            picker: PickerState::default(),
            running_apps: Vec::new(),
            update_card: UpdateCard::new(volumectl::VERSION),
            update_rx: None,
            update_url: None,
            pending_modifier: None,
            config_notice: payload.config_notice,
            help_query: String::new(),
            tray,
            tray_actions,
            primary: None,
            tray_volume,
            tray_language,
        }
    }

    /// Attach the single-instance guard so second launches can wake the Mixer.
    pub fn attach_primary(&mut self, primary: PrimaryGuard) {
        self.primary = Some(primary);
    }

    fn poll_single_instance(&mut self) {
        if self
            .primary
            .as_ref()
            .is_some_and(|guard| guard.poll_show_mixer())
        {
            self.state.show(SurfaceId::Mixer);
            self.root_shown = true;
        }
    }

    fn poll_tray(&mut self) {
        while let Ok(action) = self.tray_actions.try_recv() {
            match action {
                TrayAction::ToggleMute => self.dispatch(AppAction::ToggleMute),
                TrayAction::Reset50 => self.dispatch(AppAction::ResetVolume),
                TrayAction::ShowMixer => {
                    self.state.show(SurfaceId::Mixer);
                    self.root_shown = true;
                }
                TrayAction::ShowSettings => {
                    self.dispatch(AppAction::ShowSurface(SurfaceId::Settings));
                }
                TrayAction::ShowHelp => {
                    self.dispatch(AppAction::ShowSurface(SurfaceId::Help));
                }
                TrayAction::CheckUpdates => self.dispatch(AppAction::CheckUpdates),
                TrayAction::ReloadConfig => self.dispatch(AppAction::ReloadConfig),
                TrayAction::OpenConfig => self.dispatch(AppAction::OpenConfigLocation),
                TrayAction::Exit => {
                    self.quitting = true;
                }
                TrayAction::Header => {}
            }
        }
        if self.state.volume_percent != self.tray_volume {
            self.tray_volume = self.state.volume_percent;
            if let Some(tray) = &mut self.tray {
                tray.update_header(self.tray_volume);
            }
        }
        if self.state.language != self.tray_language {
            self.tray_language = self.state.language;
            if let Some(tray) = &mut self.tray {
                tray.set_language(self.tray_language);
            }
        }
    }

    fn language(&self) -> Language {
        self.state.language
    }

    fn dispatch(&mut self, action: AppAction) {
        self.core.handle_action(action);
    }

    /// Reflect a user gesture immediately; the confirmed core event
    /// reconciles the mirror on the next frame.
    fn apply_optimistic(&mut self, action: &AppAction) {
        match action {
            AppAction::SetVolumePercent { percent } => {
                self.state.volume_percent = (*percent).min(100) as u8;
                self.overlay_volume = self.state.volume_percent;
            }
            AppAction::AdjustVolume { delta_percent } => {
                let next = i16::from(self.state.volume_percent) + delta_percent;
                self.state.volume_percent = next.clamp(0, 100) as u8;
                self.overlay_volume = self.state.volume_percent;
            }
            AppAction::SetMute { muted } => {
                self.state.muted = *muted;
                self.overlay_muted = *muted;
            }
            AppAction::ToggleMute => {
                self.state.muted = !self.state.muted;
                self.overlay_muted = self.state.muted;
            }
            AppAction::ResetVolume => {
                self.state.volume_percent = 50;
                self.state.muted = false;
                self.overlay_volume = 50;
                self.overlay_muted = false;
            }
            AppAction::SetSessionVolume { id, percent } => {
                if let Some(session) = self.state.sessions.iter_mut().find(|s| s.id == *id) {
                    session.volume_percent = *percent;
                }
            }
            AppAction::ToggleSessionMute { id } => {
                if let Some(session) = self.state.sessions.iter_mut().find(|s| s.id == *id) {
                    session.muted = !session.muted;
                }
            }
            _ => {}
        }
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                EguiEvent::Volume { percent, muted } => {
                    let percent = percent.min(100);
                    let changed = percent != self.state.volume_percent || muted != self.state.muted;
                    self.state.volume_percent = percent;
                    self.state.muted = muted;
                    if changed {
                        // External producers (legacy hotkeys/media keys) and
                        // confirmed clicks converge here: show the HUD so the
                        // value is always visible, not only for local actions.
                        self.overlay_text = None;
                        self.overlay_volume = percent;
                        self.overlay_muted = muted;
                        self.state.show(SurfaceId::Overlay);
                        self.overlay_until = Some(
                            Instant::now()
                                + Duration::from_millis(self.config.overlay_duration_ms.max(500)),
                        );
                    }
                }
                EguiEvent::Hotkeys(status) => self.hotkey_status = status,
                EguiEvent::Sessions(sessions) => {
                    self.state.sessions = sessions.iter().map(session_state).collect();
                }
                EguiEvent::Backend(status) => {
                    self.state.status = match &status {
                        BackendStatus::Ready => UiStatus::Ready,
                        BackendStatus::Degraded { error } => UiStatus::Error(error.clone()),
                    };
                }
                EguiEvent::Open(surface) => {
                    let was_visible = self.state.is_visible(surface);
                    self.state.show(surface);
                    if surface == SurfaceId::Mixer {
                        self.root_shown = true;
                    }
                    if surface == SurfaceId::Settings && !was_visible {
                        self.settings_draft.rebase(&self.config);
                        self.settings_error = None;
                        self.recorder.cancel();
                    }
                }
                EguiEvent::Close(surface) => {
                    self.state.hide(surface);
                    if surface == SurfaceId::Mixer {
                        self.root_shown = false;
                    }
                }
                EguiEvent::Toggle(surface) => {
                    let visibility = self.state.toggle(surface);
                    if surface == SurfaceId::Mixer {
                        self.root_shown = visibility == volumectl::ui::SurfaceVisibility::Visible;
                    }
                }
                EguiEvent::Overlay {
                    text,
                    state,
                    config,
                } => {
                    log::debug!("overlay event: {:?}", text);
                    self.overlay_text = text;
                    self.overlay_volume = state.percent();
                    self.overlay_muted = state.muted;
                    self.state.volume_percent = state.percent();
                    self.state.muted = state.muted;
                    self.config = *config;
                    self.is_dark = self.resolve_dark();
                    self.state.show(SurfaceId::Overlay);
                    self.overlay_until = Some(
                        Instant::now()
                            + Duration::from_millis(self.config.overlay_duration_ms.max(500)),
                    );
                    ctx.request_repaint();
                }
                EguiEvent::ShowTrayMenu => {
                    log::debug!("tray menu requested (wired in a later task)");
                }
                EguiEvent::Exit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }
    }

    fn poll_core(&mut self) {
        if self.last_poll.elapsed() < SLOW_POLL {
            return;
        }
        self.last_poll = Instant::now();
        self.core.poll_hotkeys();
        self.core.sync_external_state();
        if self.state.is_visible(SurfaceId::Mixer)
            && self.last_sessions_poll.elapsed() >= Duration::from_millis(1_500)
        {
            self.refresh_sessions();
        }
        if self.core.reload_config_if_changed() {
            let payload = self.core.bootstrap();
            self.config = payload.config.clone();
            self.state.language = self.config.language;
            self.state.theme = self.config.appearance.theme;
            self.state.material = self.config.appearance.material;
            self.state.motion = self.config.appearance.motion;
            self.state.accent = self.config.appearance.accent;
            self.is_dark = payload.appearance.theme_resolved == "dark";
            self.applied_dark = None;
        }
    }

    fn refresh_sessions(&mut self) {
        self.last_sessions_poll = Instant::now();
        let sessions = self.core.sessions();
        self.state.sessions = sessions.iter().map(session_state).collect();
    }

    fn resolve_dark(&self) -> bool {
        match self.config.appearance.theme {
            volumectl::ui::ThemeMode::Dark => true,
            volumectl::ui::ThemeMode::Light => false,
            volumectl::ui::ThemeMode::System => self.is_dark,
        }
    }

    /// Park the root off-screen instead of hiding it, so child viewports keep
    /// receiving UI passes (eframe 0.36 skips `App::ui` for hidden roots).
    fn sync_root_visibility(&mut self, ctx: &egui::Context) {
        if self.root_shown != self.root_visible_last {
            if self.root_shown {
                let frame = frame_for(
                    SurfaceId::Mixer,
                    &WorkArea::new(0.0, 0.0, 1600.0, 900.0),
                    ctx.pixels_per_point(),
                );
                let scale = ctx.pixels_per_point();
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(egui::pos2(
                    (frame.x as f32 / scale).round(),
                    (frame.y as f32 / scale).round(),
                )));
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(
                    logical_size(SurfaceId::Mixer).0,
                    logical_size(SurfaceId::Mixer).1,
                )));
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            } else {
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(PARKED_POSITION));
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(1.0, 1.0)));
            }
            self.parked = !self.root_shown;
            self.root_visible_last = self.root_shown;
        }
        if self.parked {
            ctx.request_repaint_after(SLOW_POLL);
        }
    }

    fn handle_escape(&mut self, ui: &egui::Ui, surface: SurfaceId) {
        if surface == SurfaceId::Settings && views::capturing(&self.recorder) {
            return;
        }
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.state.hide(surface);
            self.dispatch(AppAction::HideSurface(surface));
        }
    }

    fn handle_close_request(&mut self, ui: &egui::Ui, surface: SurfaceId) {
        if !ui.input(|input| input.viewport().close_requested()) {
            return;
        }
        log::debug!("close requested for {surface:?}");
        if surface == SurfaceId::Mixer && !self.quitting {
            ui.ctx().send_viewport_cmd(ViewportCommand::CancelClose);
            self.state.hide(SurfaceId::Mixer);
            self.root_shown = false;
        } else {
            self.state.hide(surface);
        }
    }

    fn poll_updates(&mut self, ctx: &egui::Context) {
        let Some(receiver) = &self.update_rx else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(info)) => {
                self.update_url = info.release_url.clone();
                if info.is_update_available {
                    if let Some(version) = info.latest_version.clone() {
                        self.update_card.check_finished(Some(version));
                    } else {
                        self.update_card.check_finished(None);
                    }
                } else {
                    self.update_card.check_finished(None);
                }
                self.update_rx = None;
            }
            Ok(Err(error)) => {
                self.update_card.failed(error);
                self.update_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(150));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.update_card.failed("update check worker stopped");
                self.update_rx = None;
            }
        }
    }

    fn apply_settings_intent(&mut self, intent: SettingsIntent, ctx: &egui::Context) {
        match intent {
            SettingsIntent::Save => self.save_settings(),
            SettingsIntent::Cancel => {
                self.dispatch(AppAction::CancelConfig);
                self.config = self.core.config();
                self.settings_draft.rebase(&self.config);
                self.settings_error = None;
                self.recorder.cancel();
                self.pending_modifier = None;
                self.state.hide(SurfaceId::Settings);
            }
            SettingsIntent::Reset => {
                self.settings_draft.reset();
                self.pending_modifier = None;
                self.settings_error = None;
            }
            SettingsIntent::ApplyPreset(modifier) => {
                views::apply_preset(&mut self.settings_draft, modifier);
                self.pending_modifier = Some(modifier);
            }
            SettingsIntent::AddBlacklist(entry) => {
                crate::settings::blacklist::add(&mut self.settings_draft.blacklist, &entry);
            }
            SettingsIntent::RemoveBlacklist(entry) => {
                crate::settings::blacklist::remove(&mut self.settings_draft.blacklist, &entry);
            }
            SettingsIntent::ToggleBlacklist(process) => {
                crate::settings::blacklist::toggle_selected(
                    &mut self.settings_draft.blacklist,
                    &process,
                );
            }
            SettingsIntent::ClearBlacklist => {
                crate::settings::blacklist::clear(&mut self.settings_draft.blacklist);
            }
            SettingsIntent::ApplyRecommended => {
                crate::settings::blacklist::merge_recommended(
                    &mut self.settings_draft.blacklist,
                    self.config.modifier,
                );
            }
            SettingsIntent::RestoreDefaults => {
                crate::settings::blacklist::replace_defaults(
                    &mut self.settings_draft.blacklist,
                    self.config.modifier,
                );
            }
            SettingsIntent::ReloadSaved => {
                self.settings_draft.blacklist = self.config.blacklist.clone();
            }
            SettingsIntent::RefreshRunningApps => {
                self.picker.loading = false;
                self.picker.error = None;
                self.running_apps = volumectl::host_core::running_apps();
            }
            SettingsIntent::OpenConfig => {
                self.dispatch(AppAction::OpenConfigLocation);
            }
            SettingsIntent::CheckUpdates => {
                if self.update_rx.is_none() {
                    self.update_card.check_started();
                    let language = self.settings_draft.language;
                    let (sender, receiver) = mpsc::channel();
                    std::thread::spawn(move || {
                        let result = volumectl::updates::check_update_info(language);
                        let _ = sender.send(result);
                    });
                    self.update_rx = Some(receiver);
                    ctx.request_repaint_after(Duration::from_millis(150));
                }
            }
            SettingsIntent::DownloadUpdate => {
                if let Some(url) = &self.update_url {
                    open_external(url);
                }
            }
            SettingsIntent::InstallUpdate => {
                if let Some(url) = &self.update_url {
                    open_external(url);
                }
            }
            SettingsIntent::DismissUpdate => self.update_card.dismiss(),
        }
    }

    fn save_settings(&mut self) {
        if let Err(error) = self.settings_draft.validate() {
            self.settings_tab = error.tab;
            self.settings_error = Some(error);
            return;
        }
        let patch = self.settings_draft.patch();
        let autostart = self.settings_draft.autostart;
        let language = self.settings_draft.language;
        let modifier = self.pending_modifier.take();
        self.dispatch(AppAction::ApplyConfigDraft(Box::new(patch)));
        if let Some(modifier) = modifier {
            if let Err(error) = self.core.set_modifier(modifier) {
                log::warn!("modifier change rejected: {error}");
            }
        }
        if autostart != self.config.autostart {
            if let Err(error) = volumectl::autostart::set_enabled(autostart) {
                log::warn!("autostart change failed: {error}");
            }
        }
        self.config = self.core.config();
        self.settings_draft.rebase(&self.config);
        self.settings_error = None;
        self.state.language = language;
        self.is_dark = self.resolve_dark();
        self.applied_dark = None;
    }

    fn show_settings(&mut self, ui: &mut egui::Ui) {
        let tokens = style::tokens(self.is_dark);
        let notice = self
            .config_notice
            .map(|notice| views::notice_text(notice, self.settings_draft.language));
        let error = self.settings_error.clone();
        let mut intents = Vec::new();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(tokens.background)
                    .corner_radius(egui::CornerRadius::same(style::RADIUS_CARD))
                    .inner_margin(egui::Margin::same(14)),
            )
            .show(ui, |ui| {
                let mut view = views::SettingsView {
                    language: self.settings_draft.language,
                    is_dark: self.is_dark,
                    draft: &mut self.settings_draft,
                    tab: &mut self.settings_tab,
                    error: error.as_ref(),
                    recorder: &mut self.recorder,
                    picker: &mut self.picker,
                    running_apps: &self.running_apps,
                    update: &self.update_card,
                    autostart_supported: cfg!(any(
                        target_os = "windows",
                        target_os = "macos",
                        target_os = "linux"
                    )),
                    config_path: volumectl::config::config_path().display().to_string(),
                    config_notice: notice.as_deref(),
                    saved_blacklist: &self.config.blacklist,
                    modifier: self.config.modifier,
                };
                intents = views::show(ui, &mut view);
            });
        for intent in intents {
            self.apply_settings_intent(intent, ui.ctx());
        }
        self.vibrancy.apply(
            SurfaceId::Settings,
            vibrancy::material_for(SurfaceId::Settings),
            self.is_dark,
            self.state.language,
        );
    }

    fn show_mixer(&mut self, ui: &mut egui::Ui) {
        let tokens = style::tokens(self.is_dark);
        let options = crate::surfaces::mixer::MixerOptions {
            language: self.language(),
            is_dark: self.is_dark,
            green_up_to: self.config.color_thresholds.green_up_to,
            blue_up_to: self.config.color_thresholds.blue_up_to,
            app_label_style: self.config.app_label_style,
            sessions_supported: self.sessions_supported,
            error: match &self.state.status {
                UiStatus::Error(error) => Some(error.clone()),
                UiStatus::Ready | UiStatus::Syncing => None,
            },
        };
        let mut intents = Vec::new();
        let opaque = {
            let color = tokens.background;
            egui::Color32::from_rgb(color.r(), color.g(), color.b())
        };
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(opaque)
                    .corner_radius(egui::CornerRadius::same(style::RADIUS_HUD))
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ui, |ui| {
                crate::surfaces::mixer::show(
                    ui,
                    &self.state,
                    &mut self.mixer_query,
                    &options,
                    &mut intents,
                );
            });
        for intent in intents {
            match intent {
                crate::surfaces::mixer::MixerIntent::Action(action) => {
                    self.apply_optimistic(&action);
                    if matches!(action, AppAction::HideSurface(SurfaceId::Mixer)) {
                        self.state.hide(SurfaceId::Mixer);
                        self.root_shown = false;
                    }
                    self.dispatch(action);
                }
                crate::surfaces::mixer::MixerIntent::RetryBackend => {
                    self.core.sync_external_state();
                    self.refresh_sessions();
                }
            }
        }
        self.vibrancy.apply(
            SurfaceId::Mixer,
            vibrancy::material_for(SurfaceId::Mixer),
            self.is_dark,
            self.state.language,
        );
    }

    fn show_help(&mut self, ui: &mut egui::Ui) {
        let tokens = style::tokens(self.is_dark);
        let default_bindings = volumectl::config::HotkeyBindings::default();
        let bindings = self.config.hotkeys.as_ref().unwrap_or(&default_bindings);
        let mut intents = Vec::new();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(tokens.background)
                    .corner_radius(egui::CornerRadius::same(style::RADIUS_CARD))
                    .inner_margin(egui::Margin::same(14)),
            )
            .show(ui, |ui| {
                let mut view = HelpView {
                    language: self.state.language,
                    is_dark: self.is_dark,
                    query: &mut self.help_query,
                    bindings,
                    status: &self.hotkey_status,
                };
                intents = help::show(ui, &mut view);
            });
        for intent in intents {
            match intent {
                HelpIntent::OpenConfig => self.dispatch(AppAction::OpenConfigLocation),
                HelpIntent::OpenSettings => {
                    self.dispatch(AppAction::ShowSurface(SurfaceId::Settings));
                }
                HelpIntent::Close => self.state.hide(SurfaceId::Help),
            }
        }
        self.vibrancy.apply(
            SurfaceId::Help,
            vibrancy::material_for(SurfaceId::Help),
            self.is_dark,
            self.state.language,
        );
    }

    fn show_overlay(&mut self, ui: &mut egui::Ui) {
        let view = crate::surfaces::overlay::OverlayView {
            language: self.state.language,
            is_dark: self.is_dark,
            text: self.overlay_text.as_deref(),
            volume: self.overlay_volume,
            muted: self.overlay_muted,
            device: self.state.device.as_deref(),
            green_up_to: self.config.color_thresholds.green_up_to,
            blue_up_to: self.config.color_thresholds.blue_up_to,
        };
        crate::surfaces::overlay::show(ui, &view);
    }

    fn show_placeholder(&mut self, ui: &mut egui::Ui, surface: SurfaceId) {
        let language = self.language();
        let tokens = style::tokens(self.is_dark);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(tokens.background)
                    .corner_radius(egui::CornerRadius::same(style::RADIUS_CARD))
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ui, |ui| {
                let title = match surface {
                    SurfaceId::Settings => Key::SettingsTitle,
                    SurfaceId::Help => Key::HelpTitle,
                    SurfaceId::Overlay => Key::OverlayVolume,
                    SurfaceId::Mixer | SurfaceId::Tray => Key::MixerTitle,
                };
                ui.label(
                    egui::RichText::new(tr(title, language, &[]).to_string())
                        .size(16.0)
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(tr(Key::MixerSystemOutput, language, &[]).to_string())
                        .size(11.0)
                        .color(tokens.muted),
                );
            });
    }

    fn show_children(&mut self, ctx: &egui::Context) {
        let scale = ctx.pixels_per_point();
        let area = WorkArea::new(0.0, 0.0, 1600.0, 900.0);

        for surface in [SurfaceId::Settings, SurfaceId::Help] {
            if !self.state.is_visible(surface) {
                continue;
            }
            let builder = builder_for(surface, scale, &area, self.state.language);
            let id = viewport_id(surface);
            let mut requested_close = false;
            ctx.show_viewport_immediate(id, builder, |ui, _class| {
                self.handle_close_request(ui, surface);
                self.handle_escape(ui, surface);
                match surface {
                    SurfaceId::Settings => self.show_settings(ui),
                    SurfaceId::Help => self.show_help(ui),
                    _ => self.show_placeholder(ui, surface),
                }
                requested_close = ui.input(|input| input.viewport().close_requested());
            });
            if requested_close {
                self.state.hide(surface);
            }
            self.vibrancy.apply(
                surface,
                vibrancy::material_for(surface),
                self.is_dark,
                self.state.language,
            );
        }

        if self.state.is_visible(SurfaceId::Overlay) {
            let builder = builder_for(SurfaceId::Overlay, scale, &area, self.state.language);
            let id = viewport_id(SurfaceId::Overlay);
            ctx.show_viewport_immediate(id, builder, |ui, _class| {
                // Passthrough is applied after creation: requesting it in the
                // builder prevents the overlay window from appearing on
                // Windows (winit cursor-hit-test setup).
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::MousePassthrough(true));
                self.handle_escape(ui, SurfaceId::Overlay);
                self.show_overlay(ui);
            });
            self.vibrancy.apply(
                SurfaceId::Overlay,
                vibrancy::material_for(SurfaceId::Overlay),
                self.is_dark,
                self.state.language,
            );
        }
    }
}

fn open_external(url: &str) {
    #[cfg(target_os = "windows")]
    let command = ("cmd", ["/C", "start", ""].as_slice(), Some(url));
    #[cfg(target_os = "macos")]
    let command = ("open", [].as_slice(), Some(url));
    #[cfg(target_os = "linux")]
    let command = ("xdg-open", [].as_slice(), Some(url));
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let command = ("true", [].as_slice(), Some(url));
    let mut process = std::process::Command::new(command.0);
    process.args(command.1);
    if let Some(url) = command.2 {
        process.arg(url);
    }
    if let Err(error) = process.spawn() {
        log::warn!("failed to open {url}: {error}");
    }
}

fn session_state(info: &volumectl::host_core::AudioSessionInfo) -> AudioSessionState {
    AudioSessionState {
        id: info.id.clone(),
        name: info.name.clone(),
        process: info.process.clone(),
        display_name: info.display_name.clone(),
        volume_percent: info.pct,
        muted: info.muted,
        active: info.active,
        system: info.system,
    }
}

impl eframe::App for VolumeControlApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Color32::TRANSPARENT.to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.applied_dark != Some(self.is_dark) {
            style::apply(&ctx, self.is_dark, self.config.ui_font_size);
            self.applied_dark = Some(self.is_dark);
        }
        if self.applied_title_language != Some(self.state.language) {
            ctx.send_viewport_cmd(ViewportCommand::Title(vibrancy::title_for(
                SurfaceId::Mixer,
                self.state.language,
            )));
            self.applied_title_language = Some(self.state.language);
        }
        self.drain_events(&ctx);
        self.poll_core();
        self.poll_updates(&ctx);
        self.poll_single_instance();
        self.poll_tray();
        self.sync_root_visibility(&ctx);

        if self
            .overlay_until
            .is_some_and(|until| Instant::now() >= until)
        {
            self.overlay_until = None;
            self.overlay_text = None;
            self.state.hide(SurfaceId::Overlay);
        }

        if self.root_shown {
            self.handle_close_request(ui, SurfaceId::Mixer);
            self.handle_escape(ui, SurfaceId::Mixer);
            self.show_mixer(ui);
        } else {
            ctx.request_repaint_after(SLOW_POLL);
        }

        self.show_children(&ctx);

        if self.overlay_until.is_some() {
            ctx.request_repaint();
        }
        // Hotkey input never reaches egui, so keep a low-rate heartbeat to
        // drain the core channel even when no pointer/keyboard input arrives.
        ctx.request_repaint_after(SLOW_POLL);
        if self.options.smoke {
            // Smoke presentation lands with the smoke contract task.
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}
