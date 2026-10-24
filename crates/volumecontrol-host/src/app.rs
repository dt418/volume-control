//! Winit adapter: windows, input translation, state-driven rendering loop.

use std::time::{Duration, Instant, SystemTime};

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use liquid_glass::a11y::{A11yAdapter, A11yTreeBuilder};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::Window;

use liquid_glass::input::hit_test;
use liquid_glass::scene::{Point, Scene};
use liquid_glass::surfaces::{HELP_CLOSE_BTN_ID, MASTER_SLIDER_ID};
use liquid_glass::LiquidGlassSurfaceController;

use volumectl::linux_host_core::HostCore;
use volumectl::ui::{AppAction, MotionMode, SurfaceId};

use crate::paint::{FrameRenderer, SurfaceView};
use crate::placement::{self, WorkArea};

/// Cadence for external state polling and config reload checks.
const SLOW_POLL: Duration = Duration::from_millis(150);
/// Animation frame cadence while springs or toasts are active.
const FRAME_BUDGET: Duration = Duration::from_millis(16);

/// One open native surface window plus its GPU view and cached cursor.
struct SurfaceWindow {
    id: SurfaceId,
    window: Arc<Window>,
    view: SurfaceView,
    a11y: A11yAdapter,
    focus: liquid_glass::input::FocusManager,
    ime: liquid_glass::input::ImeState,
    modifiers: winit::keyboard::ModifiersState,
    cursor: Option<(f64, f64)>,
}

use std::sync::Arc;

/// Return the complete accessibility tree for one initial request.
struct InitialTree(TreeUpdate);

impl ActivationHandler for InitialTree {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        Some(self.0.clone())
    }
}

/// Forward AccessKit actions through the same semantic hit-test path as
/// pointer input. Unsupported requests intentionally do nothing.
struct ActionBridge {
    window_id: winit::window::WindowId,
    sender: std::sync::mpsc::Sender<(winit::window::WindowId, ActionRequest)>,
}

impl ActionHandler for ActionBridge {
    fn do_action(&mut self, request: ActionRequest) {
        let _ = self.sender.send((self.window_id, request));
    }
}

/// Accessibility may be disabled by the OS without affecting native windows.
struct NoopDeactivation;

impl DeactivationHandler for NoopDeactivation {
    fn deactivate_accessibility(&mut self) {}
}

/// The Winit application driving every Liquid Glass surface.
pub struct HostApp {
    core: HostCore,
    controller: LiquidGlassSurfaceController,
    renderer: Option<FrameRenderer>,
    windows: Vec<SurfaceWindow>,
    scenes: Vec<(SurfaceId, Scene)>,
    overlay_animating: bool,
    dirty: bool,
    accessibility_actions: std::sync::mpsc::Receiver<(winit::window::WindowId, ActionRequest)>,
    accessibility_sender: std::sync::mpsc::Sender<(winit::window::WindowId, ActionRequest)>,
    smoke_requested: bool,
    show_all: bool,
    target_surface: Option<SurfaceId>,
    smoke_seen: std::collections::HashSet<SurfaceId>,
    smoke_started: Option<Instant>,
    smoke_duration: Duration,
    smoke_verified: bool,
    last_poll: Instant,
    last_config_mtime: Option<SystemTime>,
    initialized: bool,
}

impl HostApp {
    /// Construct the adapter around a configured host reducer.
    #[must_use]
    pub fn new(
        core: HostCore,
        smoke: bool,
        show_all: bool,
        target_surface: Option<SurfaceId>,
        last_config_mtime: Option<SystemTime>,
        smoke_duration_ms: u64,
    ) -> Self {
        let initial = core.state().clone();
        let controller = LiquidGlassSurfaceController::new(&initial, Instant::now());
        let (accessibility_sender, accessibility_actions) = std::sync::mpsc::channel();
        Self {
            core,
            controller,
            renderer: None,
            windows: Vec::new(),
            scenes: Vec::new(),
            overlay_animating: false,
            dirty: true,
            accessibility_actions,
            accessibility_sender,
            smoke_requested: smoke,
            show_all,
            target_surface,
            smoke_seen: std::collections::HashSet::new(),
            smoke_started: None,
            smoke_duration: Duration::from_millis(smoke_duration_ms),
            smoke_verified: !smoke,
            last_poll: Instant::now(),
            last_config_mtime,
            initialized: false,
        }
    }

    pub fn smoke_result(&self) -> Result<(), String> {
        if !self.smoke_requested {
            return Ok(());
        }
        if self.core.quit_requested() && self.smoke_verified {
            Ok(())
        } else {
            Err("host smoke actions did not complete through the production loop".into())
        }
    }

    fn work_area(&self, event_loop: &ActiveEventLoop) -> WorkArea {
        let Some(monitor) = event_loop.primary_monitor() else {
            return WorkArea::new(0.0, 0.0, 1600.0, 900.0);
        };
        let position = monitor.position();
        let size = monitor.size();
        // Physical monitor extents converted to logical work area. Taskbar
        // exclusion is approximated by the overlay tray clearance.
        WorkArea::new(
            position.x as f32,
            position.y as f32,
            size.width as f32,
            size.height as f32 * 0.92,
        )
    }

    fn open_surface(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: SurfaceId,
        area: &WorkArea,
    ) -> Result<(), liquid_glass::Error> {
        if self.windows.iter().any(|entry| entry.id == id) {
            return Ok(());
        }
        let renderer = self.renderer.as_mut().ok_or_else(|| {
            liquid_glass::Error::Gpu("renderer unavailable before window creation".into())
        })?;
        let scale = event_loop
            .primary_monitor()
            .map(|monitor| monitor.scale_factor() as f32)
            .unwrap_or(1.0);
        let frame = placement::frame_for(id, area, scale);
        let hud = matches!(id, SurfaceId::Overlay | SurfaceId::Mixer);
        let mut attributes = Window::default_attributes()
            .with_title(surface_title(id))
            .with_decorations(false)
            .with_transparent(true)
            .with_visible(false)
            .with_resizable(matches!(id, SurfaceId::Settings))
            .with_inner_size(LogicalSize::new(
                placement::logical_size(id).0,
                placement::logical_size(id).1,
            ));
        if id == SurfaceId::Settings {
            attributes = attributes.with_min_inner_size(LogicalSize::new(620.0, 520.0));
        }
        if hud {
            attributes = attributes.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
            #[cfg(target_os = "windows")]
            {
                use winit::platform::windows::WindowAttributesExtWindows;
                attributes = attributes.with_skip_taskbar(true);
            }
        }
        let window = Arc::new(event_loop.create_window(attributes).map_err(|error| {
            log::error!("native window creation failed for {id:?}: {error}");
            liquid_glass::Error::Surface(error.to_string())
        })?);
        window.set_outer_position(winit::dpi::PhysicalPosition::new(frame.x, frame.y));
        let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(frame.width, frame.height));
        log::info!(
            "created window {id:?}: frame={:?} outer={:?} inner={:?}",
            frame,
            window.outer_position(),
            window.inner_size(),
        );
        let initial_tree = A11yTreeBuilder::build(&Scene::new(), None, surface_title(id));
        let a11y = A11yAdapter::new(
            event_loop,
            &window,
            InitialTree(initial_tree),
            ActionBridge {
                window_id: window.id(),
                sender: self.accessibility_sender.clone(),
            },
            NoopDeactivation,
        );
        window.set_ime_allowed(true);
        let material = self.core.config().appearance.material;
        let is_dark = self.core.config().appearance.theme != volumectl::ui::ThemeMode::Light;
        crate::vibrancy::apply_vibrancy_for_surface(&window, id, material, is_dark);
        window.set_visible(true);
        let view = renderer.create_view(&window).map_err(|error| {
            log::error!("GPU surface creation failed for {id:?}: {error}");
            liquid_glass::Error::Surface(error.to_string())
        })?;
        self.windows.push(SurfaceWindow {
            id,
            window,
            view,
            a11y,
            focus: liquid_glass::input::FocusManager::new(),
            ime: liquid_glass::input::ImeState::new(),
            modifiers: winit::keyboard::ModifiersState::empty(),
            cursor: None,
        });
        Ok(())
    }

    fn close_surface(&mut self, id: SurfaceId) {
        self.windows.retain(|entry| entry.id != id);
        self.scenes.retain(|(scene_id, _)| *scene_id != id);
    }

    fn sync_windows(&mut self, event_loop: &ActiveEventLoop) {
        let open: Vec<SurfaceId> = self.windows.iter().map(|entry| entry.id).collect();
        let state = self.core.state().clone();
        let area = self.work_area(event_loop);
        for op in crate::sync::plan(&open, &state) {
            self.dirty = true;
            match op {
                crate::sync::SurfaceOp::Open(id) => {
                    if let Err(error) = self.open_surface(event_loop, id, &area) {
                        log::error!("open surface {id:?} failed: {error}");
                        if self.smoke_requested {
                            event_loop.exit();
                        }
                    }
                }
                crate::sync::SurfaceOp::Close(id) => self.close_surface(id),
            }
        }
    }

    fn reduce_motion(&self) -> bool {
        matches!(self.core.config().appearance.motion, MotionMode::Reduced)
    }

    fn rebuild_scenes(&mut self, now: Instant) {
        let state = self.core.state().clone();
        self.controller.publish(&state, now, self.reduce_motion());
        self.overlay_animating = self.controller.update_overlay(now);
        self.scenes.clear();

        if state.is_visible(SurfaceId::Overlay) && self.controller.overlay().is_visible() {
            let mut scene = Scene::new();
            self.controller.populate_overlay_scene(&mut scene, now);
            self.scenes.push((SurfaceId::Overlay, scene));
        }
        if state.is_visible(SurfaceId::Mixer) {
            let mut scene = Scene::new();
            self.controller.populate_mixer_scene(&mut scene);
            self.scenes.push((SurfaceId::Mixer, scene));
        }
        if state.is_visible(SurfaceId::Settings) {
            let mut scene = Scene::new();
            let (w, h) = self
                .windows
                .iter()
                .find(|entry| entry.id == SurfaceId::Settings)
                .map(|entry| {
                    let size = entry.window.inner_size();
                    let scale = entry.window.scale_factor() as f32;
                    (size.width as f32 / scale, size.height as f32 / scale)
                })
                .unwrap_or_else(|| placement::logical_size(SurfaceId::Settings));
            self.controller.populate_settings_scene(&mut scene, w, h);
            self.scenes.push((SurfaceId::Settings, scene));
        }
        if state.is_visible(SurfaceId::Help) {
            let mut scene = Scene::new();
            self.controller.populate_help_scene(&mut scene);
            self.scenes.push((SurfaceId::Help, scene));
        }
        for (id, _) in &self.scenes {
            self.smoke_seen.insert(*id);
        }
        if self.smoke_seen.contains(&SurfaceId::Mixer)
            && self.smoke_seen.contains(&SurfaceId::Settings)
            && self.smoke_seen.contains(&SurfaceId::Help)
        {
            self.smoke_verified = true;
        }
        self.publish_accessibility_trees();
    }

    fn publish_accessibility_trees(&mut self) {
        for entry in &mut self.windows {
            if let Some((_, scene)) = self.scenes.iter().find(|(id, _)| *id == entry.id) {
                let tree =
                    A11yTreeBuilder::build(scene, entry.focus.focused(), surface_title(entry.id));
                entry.a11y.update_if_active(|| tree);
            }
        }
    }

    fn pump_host(&mut self, now: Instant) {
        self.core.poll_hotkeys();
        self.core.refresh_audio();
        if self.core.quit_requested() {
            return;
        }
        self.dirty = true;
        let config_path = volumectl::config::config_path();
        let current_mtime = std::fs::metadata(&config_path)
            .and_then(|metadata| metadata.modified())
            .ok();
        if current_mtime != self.last_config_mtime {
            self.last_config_mtime = current_mtime;
            match self.core.reload_config_at(&config_path, current_mtime) {
                Ok(()) => self.sync_renderer_theme(),
                Err(error) => log::warn!("host config reload ignored: {error}"),
            }
        }
        if let Some(message) = self.core.update_message.take() {
            self.controller.show_notification(message, now);
            self.dirty = true;
        }
        if self.show_all && !self.controller.overlay().is_visible() {
            self.controller
                .show_notification("Native overlay preview", now);
        }
        if self.smoke_requested && !self.smoke_verified {
            if !self.smoke_seen.contains(&SurfaceId::Mixer) {
                self.core
                    .apply_action(&AppAction::ShowSurface(SurfaceId::Mixer));
            } else if !self.smoke_seen.contains(&SurfaceId::Settings) {
                self.core
                    .apply_action(&AppAction::ShowSurface(SurfaceId::Settings));
            } else if !self.smoke_seen.contains(&SurfaceId::Help) {
                self.core
                    .apply_action(&AppAction::ShowSurface(SurfaceId::Help));
            }
        }
        self.sync_renderer_theme();
        self.rebuild_scenes(now);
        self.sync_overlay_visibility();
    }

    fn sync_renderer_theme(&mut self) {
        let theme = self.core.config().appearance.theme;
        let material = self.core.config().appearance.material;
        let motion = self.core.config().appearance.motion;
        let capabilities = *self.core.capabilities();
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_preferences(theme, material, motion, &capabilities);
        }
    }

    fn sync_overlay_visibility(&mut self) {
        let overlay_open = self.controller.overlay().is_visible();
        if let Some(entry) = self
            .windows
            .iter()
            .find(|entry| entry.id == SurfaceId::Overlay)
        {
            if entry.window.is_visible() != Some(overlay_open) {
                entry.window.set_visible(overlay_open);
            }
        }
    }
    fn request_redraws(&self, event_loop: &ActiveEventLoop) {
        if !self.dirty && !self.overlay_animating && !self.smoke_started.is_some() {
            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + SLOW_POLL));
            return;
        }
        for (id, _) in &self.scenes {
            if let Some(entry) = self.windows.iter().find(|entry| entry.id == *id) {
                entry.window.request_redraw();
            }
        }
        let animating = self.controller.overlay().is_visible();
        let next = if self.scenes.is_empty() && !animating {
            SLOW_POLL
        } else {
            FRAME_BUDGET
        };
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + next));
    }

    /// Render dirty scenes directly from the main event loop.
    ///
    /// Winit may defer WM_PAINT while new redraws are continuously requested;
    /// this path guarantees every open surface gets a frame, not just the one
    /// whose WM_PAINT happened to be delivered first.
    fn render_pending(&mut self) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let mut rebuild = false;
        for entry in &mut self.windows {
            if let Some((_, scene)) = self.scenes.iter().find(|(id, _)| *id == entry.id) {
                if let Err(error) = renderer.render(&entry.window, &mut entry.view, scene) {
                    log::error!("render failed for {:?}: {error}", entry.id);
                    if matches!(error, liquid_glass::Error::Gpu(_)) {
                        rebuild = true;
                        break;
                    }
                } else {
                    log::debug!("main-loop render complete {:?}", entry.id);
                }
            }
        }
        if rebuild {
            self.renderer = None;
            self.rebuild_renderer();
        }
    }

    fn rebuild_renderer(&mut self) {
        match FrameRenderer::new(self.core.config().appearance.theme) {
            Ok(renderer) => {
                self.renderer = Some(renderer);
                self.windows.clear();
                self.scenes.clear();
                self.rebuild_scenes(Instant::now());
            }
            Err(error) => {
                log::error!("GPU rebuild failed: {error}");
            }
        }
    }

    fn handle_pointer(&mut self, id: SurfaceId, logical_x: f32, logical_y: f32) {
        let scene = self
            .scenes
            .iter()
            .find(|(scene_id, _)| *scene_id == id)
            .map(|(_, scene)| scene);
        let Some(scene) = scene else { return };
        let Some(semantic) = hit_test(scene, Point::new(logical_x, logical_y)) else {
            return;
        };
        if id == SurfaceId::Mixer {
            if let Some(track) = scene.get(semantic).map(|node| node.layout_bounds) {
                if semantic == MASTER_SLIDER_ID || semantic.0 >= 1000 {
                    let fraction = ((logical_x - track.x) / track.width.max(1.0)).clamp(0.0, 1.0);
                    if let Some(action) = self
                        .controller
                        .mixer()
                        .handle_slider_drag(semantic, fraction)
                    {
                        self.core.apply_action(&action);
                    }
                    return;
                }
            }
        }

        self.dispatch_semantic_action(id, semantic);
    }

    fn handle_keyboard(
        &mut self,
        id: SurfaceId,
        window_index: usize,
        key: &winit::keyboard::Key,
        shift: bool,
    ) {
        use winit::keyboard::NamedKey;

        let scene = self
            .scenes
            .iter()
            .find(|(scene_id, _)| *scene_id == id)
            .map(|(_, scene)| scene);
        let Some(scene) = scene else { return };

        match key {
            winit::keyboard::Key::Named(NamedKey::Escape) => {
                self.core.apply_action(&AppAction::HideSurface(id));
                self.dirty = true;
                self.rebuild_scenes(Instant::now());
            }
            winit::keyboard::Key::Named(NamedKey::Tab) => {
                let target = if shift {
                    self.windows[window_index].focus.focus_prev(scene)
                } else {
                    self.windows[window_index].focus.focus_next(scene)
                };
                if let Some(target) = target {
                    log::debug!("tab moved focus to {target:?} on {id:?}");
                    self.dirty = true;
                    self.publish_accessibility_trees();
                }
            }
            winit::keyboard::Key::Named(NamedKey::Space)
                if id == SurfaceId::Mixer
                    && self.windows[window_index].focus.focused()
                        == Some(liquid_glass::surfaces::SEARCH_INPUT_ID) =>
            {
                self.controller.mixer_mut().append_search_char(' ');
                self.dirty = true;
                self.rebuild_scenes(Instant::now());
            }
            winit::keyboard::Key::Named(NamedKey::Enter | NamedKey::Space) => {
                if let Some(focused) = self.windows[window_index].focus.focused() {
                    self.dispatch_semantic_action(id, focused);
                }
            }
            winit::keyboard::Key::Named(NamedKey::Backspace) if id == SurfaceId::Mixer => {
                self.controller.mixer_mut().pop_search_char();
                self.dirty = true;
                self.rebuild_scenes(Instant::now());
            }
            winit::keyboard::Key::Character(s) if id == SurfaceId::Mixer => {
                self.controller.mixer_mut().commit_search_text(s.as_str());
                self.dirty = true;
                self.rebuild_scenes(Instant::now());
            }
            winit::keyboard::Key::Named(NamedKey::Backspace)
            | winit::keyboard::Key::Character(_) => {}
            _ => {}
        }
    }

    fn handle_ime(&mut self, id: SurfaceId, action: liquid_glass::input::ImeAction) {
        if id == SurfaceId::Mixer {
            if let liquid_glass::input::ImeAction::Commit(text) = action {
                self.controller.mixer_mut().commit_search_text(&text);
                self.dirty = true;
                self.rebuild_scenes(Instant::now());
            }
        }
    }

    fn handle_accessibility_action(&mut self, id: SurfaceId, request: ActionRequest) {
        if matches!(
            request.action,
            accesskit::Action::Increment | accesskit::Action::Decrement
        ) && id == SurfaceId::Mixer
        {
            let semantic = liquid_glass::scene::SemanticId(request.target_node.0);
            let step = if request.action == accesskit::Action::Increment {
                1.0
            } else {
                -1.0
            };
            if let Some(action) = self
                .controller
                .mixer()
                .handle_slider_adjustment(semantic, step)
            {
                self.core.apply_action(&action);
            }
            return;
        }
        if request.action != accesskit::Action::Click {
            return;
        }
        self.dispatch_semantic_action(id, liquid_glass::scene::SemanticId(request.target_node.0));
    }

    fn dispatch_semantic_action(
        &mut self,
        id: SurfaceId,
        semantic: liquid_glass::scene::SemanticId,
    ) {
        let action = match id {
            SurfaceId::Mixer => self.controller.mixer().handle_click(semantic),
            SurfaceId::Settings => self.controller.settings_mut().handle_click(semantic),
            SurfaceId::Help => {
                (semantic == HELP_CLOSE_BTN_ID).then_some(AppAction::HideSurface(SurfaceId::Help))
            }
            SurfaceId::Overlay | SurfaceId::Tray => None,
        };
        if let Some(action) = action {
            self.core.apply_action(&action);
        }
        self.dirty = true;
        self.rebuild_scenes(Instant::now());
    }
}

fn surface_title(id: SurfaceId) -> &'static str {
    match id {
        SurfaceId::Overlay => "VolumeControl Overlay",
        SurfaceId::Mixer => "VolumeControl Mixer",
        SurfaceId::Settings => "VolumeControl Settings",
        SurfaceId::Help => "VolumeControl Help",
        SurfaceId::Tray => "VolumeControl Tray",
    }
}

impl ApplicationHandler for HostApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        event_loop.set_control_flow(ControlFlow::Wait);
        match FrameRenderer::new(self.core.config().appearance.theme) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => {
                log::error!("GPU init failed: {error}");
                event_loop.exit();
                return;
            }
        }
        if let Some(target) = self.target_surface {
            self.core.apply_action(&AppAction::ShowSurface(target));
        } else if self.smoke_requested || self.show_all {
            // Present every surface once. Smoke mode exits after duration,
            // while show_all remains interactive for debug and testing.
            for id in [
                SurfaceId::Overlay,
                SurfaceId::Mixer,
                SurfaceId::Settings,
                SurfaceId::Help,
            ] {
                self.core.apply_action(&AppAction::ShowSurface(id));
            }
            if self.smoke_requested {
                self.core.update_message = Some("Smoke: native update toast".into());
                self.smoke_started = Some(Instant::now());
            }
        } else {
            // HUD visibility follows controller springs; the state flag only
            // needs to be visible once so volume changes trigger the HUD.
            self.core
                .apply_action(&AppAction::ShowSurface(SurfaceId::Overlay));
        }
        self.sync_windows(event_loop);
        self.pump_host(Instant::now());
        self.request_redraws(event_loop);
        self.render_pending();
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(index) = self
            .windows
            .iter()
            .position(|entry| entry.window.id() == window_id)
        else {
            return;
        };
        let window = self.windows[index].window.clone();
        self.windows[index].a11y.process_event(&window, &event);
        match event {
            WindowEvent::RedrawRequested => {
                let Some(renderer) = self.renderer.as_mut() else {
                    return;
                };
                let id = self.windows[index].id;
                let window = self.windows[index].window.clone();
                let view = &mut self.windows[index].view;
                if let Some((_, scene)) = self.scenes.iter().find(|(scene_id, _)| *scene_id == id) {
                    if let Err(error) = renderer.render(&window, view, scene) {
                        log::error!("render failed for {id:?}: {error}");
                        if matches!(error, liquid_glass::Error::Gpu(_)) {
                            self.renderer = None;
                            self.rebuild_renderer();
                        }
                    }
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.rebuild_scenes(Instant::now());
                self.dirty = true;
            }
            WindowEvent::CloseRequested => {
                let id = self.windows[index].id;
                self.core.apply_action(&AppAction::HideSurface(id));
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.windows[index].cursor = Some((position.x, position.y));
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.windows[index].modifiers = modifiers.state();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let (id, cursor, scale) = {
                    let entry = &self.windows[index];
                    (entry.id, entry.cursor, entry.window.scale_factor() as f32)
                };
                if let Some((x, y)) = cursor {
                    self.handle_pointer(
                        id,
                        (x / f64::from(scale)) as f32,
                        (y / f64::from(scale)) as f32,
                    );
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        state: ElementState::Pressed,
                        logical_key,
                        ..
                    },
                ..
            } => {
                let id = self.windows[index].id;
                let shift = self.windows[index].modifiers.shift_key();
                self.handle_keyboard(id, index, &logical_key, shift);
            }
            WindowEvent::Ime(ime_event) => {
                let id = self.windows[index].id;
                let action = self.windows[index].ime.handle_event(&ime_event);
                self.handle_ime(id, action);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if self.core.quit_requested() {
            event_loop.exit();
            return;
        }
        if now.duration_since(self.last_poll) >= SLOW_POLL || self.smoke_started.is_some() {
            self.last_poll = now;
            self.pump_host(now);
        }
        while let Ok((window_id, request)) = self.accessibility_actions.try_recv() {
            if let Some(entry) = self
                .windows
                .iter()
                .find(|entry| entry.window.id() == window_id)
            {
                let id = entry.id;
                self.handle_accessibility_action(id, request);
            }
        }
        self.sync_windows(event_loop);
        self.request_redraws(event_loop);
        if self.dirty || self.overlay_animating || self.smoke_started.is_some() {
            self.render_pending();
            self.dirty = false;
        }

        if let Some(started) = self.smoke_started {
            if started.elapsed() >= self.smoke_duration {
                self.core.apply_action(&AppAction::Exit);
                if self.core.quit_requested() {
                    event_loop.exit();
                }
            }
        }
    }
}
