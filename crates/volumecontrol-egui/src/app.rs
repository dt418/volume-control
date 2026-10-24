//! eframe application: root Mixer viewport, immediate child viewports, and
//! the AppCore bridge loop.

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{ViewportBuilder, ViewportCommand, ViewportId};

use volumectl::config::{Config, Language};
use volumectl::host_core::{AppCore, BackendStatus};
use volumectl::hotkeys::HotkeyRegResult;
use volumectl::ui::{AppAction, AppState, AudioSessionState, SurfaceId, UiStatus};

use crate::i18n::{tr, Key};
use crate::placement::{frame_for, logical_size, WorkArea};
use crate::sink::EguiEvent;
use crate::style;
use crate::vibrancy::{self, VibrancyState};

/// Cadence for hotkey/audio/config polling, matching the legacy host timer.
const SLOW_POLL: Duration = Duration::from_millis(150);

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
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            smoke: false,
            smoke_ms: 2_500,
            show_all: false,
            target_surface: None,
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
pub fn builder_for(surface: SurfaceId, scale: f32, area: &WorkArea) -> ViewportBuilder {
    let frame = frame_for(surface, area, scale);
    let (logical_w, logical_h) = logical_size(surface);
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let builder = ViewportBuilder::default()
        .with_title(vibrancy::title_for(surface))
        .with_inner_size(egui::vec2(logical_w, logical_h))
        .with_position(egui::pos2(
            (frame.x as f32 / scale).round(),
            (frame.y as f32 / scale).round(),
        ));
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
            .with_always_on_top()
            .with_taskbar(false)
            .with_mouse_passthrough(true),
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
    last_poll: Instant,
    overlay_text: Option<String>,
    overlay_until: Option<Instant>,
    parked: bool,
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
            last_poll: Instant::now(),
            overlay_text: None,
            overlay_until: None,
            parked: false,
        }
    }

    fn language(&self) -> Language {
        self.state.language
    }

    fn dispatch(&mut self, action: AppAction) {
        self.core.handle_action(action);
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                EguiEvent::Volume { percent, muted } => {
                    self.state.volume_percent = percent.min(100);
                    self.state.muted = muted;
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
                    self.state.show(surface);
                    if surface == SurfaceId::Mixer {
                        self.root_shown = true;
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
                    self.overlay_text = text;
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

    fn show_mixer(&mut self, ui: &mut egui::Ui) {
        let language = self.language();
        let tokens = style::tokens(self.is_dark);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(tokens.background)
                    .corner_radius(egui::CornerRadius::same(style::RADIUS_HUD))
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(tr(Key::MixerTitle, language, &[]).to_string())
                            .size(13.0)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}%",
                            tr(Key::CommonVolume, language, &[]),
                            self.state.volume_percent
                        ))
                        .size(11.0)
                        .color(tokens.muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let close = ui
                            .add(egui::Button::new(egui_phosphor::regular::X).frame(false))
                            .on_hover_text(tr(Key::CommonClose, language, &[]).to_string());
                        if close.clicked() {
                            self.state.hide(SurfaceId::Mixer);
                            self.root_shown = false;
                        }
                        let settings = ui
                            .add(egui::Button::new(egui_phosphor::regular::GEAR).frame(false))
                            .on_hover_text(tr(Key::SettingsTitle, language, &[]).to_string());
                        if settings.clicked() {
                            self.dispatch(AppAction::ShowSurface(SurfaceId::Settings));
                        }
                        let help = ui
                            .add(egui::Button::new(egui_phosphor::regular::QUESTION).frame(false))
                            .on_hover_text(tr(Key::HelpTitle, language, &[]).to_string());
                        if help.clicked() {
                            self.dispatch(AppAction::ShowSurface(SurfaceId::Help));
                        }
                    });
                });
                ui.add_space(8.0);
                if ui
                    .button(tr(Key::MixerMute, language, &[]).to_string())
                    .clicked()
                {
                    self.dispatch(AppAction::ToggleMute);
                }
                ui.add(
                    egui::Slider::new(&mut self.state.volume_percent, 0..=100)
                        .text(tr(Key::MixerSystemOutput, language, &[]).to_string()),
                );
                if let UiStatus::Error(error) = &self.state.status {
                    ui.colored_label(tokens.danger, error);
                }
            });
        self.vibrancy.apply(
            SurfaceId::Mixer,
            vibrancy::material_for(SurfaceId::Mixer),
            self.is_dark,
        );
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

    fn show_overlay_text(&mut self, ui: &mut egui::Ui, text: &str) {
        let tokens = style::tokens(self.is_dark);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(tokens.background)
                    .corner_radius(egui::CornerRadius::same(style::RADIUS_HUD))
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(text.to_string())
                        .size(12.0)
                        .color(tokens.text),
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
            log::debug!("showing immediate viewport {surface:?}");
            let builder = builder_for(surface, scale, &area);
            let id = viewport_id(surface);
            let mut requested_close = false;
            ctx.show_viewport_immediate(id, builder, |ui, _class| {
                self.handle_close_request(ui, surface);
                self.handle_escape(ui, surface);
                self.show_placeholder(ui, surface);
                requested_close = ui.input(|input| input.viewport().close_requested());
            });
            if requested_close {
                self.state.hide(surface);
            }
            self.vibrancy
                .apply(surface, vibrancy::material_for(surface), self.is_dark);
        }

        if self.state.is_visible(SurfaceId::Overlay) {
            let builder = builder_for(SurfaceId::Overlay, scale, &area);
            let id = viewport_id(SurfaceId::Overlay);
            ctx.show_viewport_immediate(id, builder, |ui, _class| {
                self.handle_escape(ui, SurfaceId::Overlay);
                match self.overlay_text.clone() {
                    Some(text) => self.show_overlay_text(ui, &text),
                    None => self.show_placeholder(ui, SurfaceId::Overlay),
                }
            });
            self.vibrancy.apply(
                SurfaceId::Overlay,
                vibrancy::material_for(SurfaceId::Overlay),
                self.is_dark,
            );
        }
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
            style::apply(&ctx, self.is_dark);
            self.applied_dark = Some(self.is_dark);
        }
        self.drain_events(&ctx);
        self.poll_core();
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
        if self.options.smoke {
            // Smoke presentation lands with the smoke contract task.
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}
