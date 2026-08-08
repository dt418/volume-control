//! Linux application host (GTK4 + PulseAudio).
//!
//! The platform-neutral reducer lives in [`crate::linux_host_core`]. This file
//! only owns GTK initialization, display capability detection, GLib polling,
//! and the concrete Linux renderer/backend adapters.

#[cfg(all(target_os = "linux", feature = "gtk-renderer"))]
mod gtk_host {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::sync::mpsc::{channel, Receiver};
    use std::time::{Duration, Instant, SystemTime};

    use gtk::gdk;
    use gtk::prelude::*;
    use gtk4 as gtk;

    use crate::audio::{default_backend, AudioBackend, AudioError, VolumeState};
    use crate::config;
    use crate::hotkeys_rdev::RdevHotkeys;
    use crate::linux_host_core::{HostCore, HotkeySource};
    use crate::ui::platform::linux::renderer::LinuxRenderer;
    use crate::ui::{tokens_for, AppAction, HostHandle, NativeRenderer, UiCapabilities, WorkArea};

    const FAST_POLL_MS: u64 = 15;
    const SLOW_POLL_MS: u64 = 150;
    const _: () = assert!(FAST_POLL_MS < SLOW_POLL_MS);

    fn fallback_caps() -> UiCapabilities {
        UiCapabilities {
            compositor: false,
            blur: false,
            high_contrast: false,
            reduced_motion: false,
            dpi_scale: 1.0,
            work_area: WorkArea::new(0, 0, 1600, 900),
        }
    }

    fn display() -> Option<gdk::Display> {
        gdk::Display::default()
    }

    fn is_x11() -> bool {
        display()
            .map(|display| display.backend().is_x11())
            .unwrap_or(false)
    }

    fn is_wayland() -> bool {
        display()
            .map(|display| display.backend().is_wayland())
            .unwrap_or(false)
    }

    fn detect_caps() -> UiCapabilities {
        let fallback = fallback_caps();
        let Some(display) = display() else {
            return fallback;
        };
        let monitors = display.monitors();
        let Some(monitor) = monitors.item(0).and_downcast::<gdk::Monitor>() else {
            return fallback;
        };
        let geometry = monitor.geometry();
        UiCapabilities {
            compositor: display.backend().is_wayland(),
            blur: display.backend().is_wayland(),
            high_contrast: false,
            reduced_motion: false,
            dpi_scale: monitor.scale_factor().max(1) as f32,
            work_area: WorkArea::new(
                geometry.x(),
                geometry.y(),
                geometry.width(),
                geometry.height(),
            ),
        }
    }

    fn file_mtime() -> Option<SystemTime> {
        std::fs::metadata(config::config_path())
            .ok()
            .and_then(|metadata| metadata.modified().ok())
    }

    fn publish(core: &HostCore, renderer: &mut LinuxRenderer) {
        let tokens = tokens_for(
            core.state().theme,
            core.capabilities().high_contrast,
            core.config().appearance.accent,
            || Some(core.capabilities().reduced_motion),
        );
        renderer.publish(core.state(), &tokens, core.capabilities());
    }

    fn poll_fast(core: &mut HostCore, receiver: &Receiver<AppAction>) {
        core.poll_hotkeys();
        while let Ok(action) = receiver.try_recv() {
            core.apply_action(&action);
        }
    }

    fn poll_slow(core: &mut HostCore, renderer: &mut LinuxRenderer) {
        let current_mtime = file_mtime();
        if core.take_config_reload_request() || current_mtime != core.config_mtime() {
            match core.reload_config_at(&config::config_path(), current_mtime) {
                Ok(()) => core.set_config_mtime(current_mtime),
                Err(error) => log::warn!("Linux config reload ignored: {error}"),
            }
        }
        if core.can_retry_audio() {
            core.record_audio_retry();
            match default_backend() {
                Ok(audio) => core.set_audio_backend(audio),
                Err(error) => {
                    log::warn!("Linux audio retry failed: {error}");
                    core.mark_degraded(format!("audio: {error}"));
                }
            }
        }
        core.refresh_audio();
        publish(core, renderer);
    }

    fn run_loop(
        audio: Option<Box<dyn AudioBackend>>,
        audio_error: Option<String>,
        hotkeys: Option<Box<dyn HotkeySource>>,
        hotkey_error: Option<String>,
        config: config::Config,
        caps: UiCapabilities,
        smoke: bool,
    ) -> Result<(), String> {
        let (sender, receiver) = channel::<AppAction>();
        let host = HostHandle::new(move |action| {
            let _ = sender.send(action);
        });
        let mut renderer = LinuxRenderer::create(host, caps).map_err(|error| error.to_string())?;
        let mut core = HostCore::new(audio, hotkeys, config, caps);
        if let Some(error) = audio_error {
            core.mark_degraded(error);
        }
        if let Some(error) = hotkey_error {
            core.mark_degraded(error);
        }
        core.set_config_mtime(file_mtime());
        publish(&core, &mut renderer);

        let main_loop = gtk::glib::MainLoop::new(None, false);
        let state = Rc::new(RefCell::new(Some((core, renderer))));
        let receiver = Rc::new(receiver);

        let fast_loop = main_loop.clone();
        let fast_state = Rc::clone(&state);
        let fast_receiver = Rc::clone(&receiver);
        let fast_source =
            gtk::glib::timeout_add_local(Duration::from_millis(FAST_POLL_MS), move || {
                let mut state = fast_state.borrow_mut();
                let Some((core, _renderer)) = state.as_mut() else {
                    return gtk::glib::ControlFlow::Break;
                };
                poll_fast(core, &fast_receiver);
                if core.quit_requested() {
                    fast_loop.quit();
                }
                gtk::glib::ControlFlow::Continue
            });

        let slow_loop = main_loop.clone();
        let slow_state = Rc::clone(&state);
        let slow_source =
            gtk::glib::timeout_add_local(Duration::from_millis(SLOW_POLL_MS), move || {
                let mut state = slow_state.borrow_mut();
                let Some((core, renderer)) = state.as_mut() else {
                    return gtk::glib::ControlFlow::Break;
                };
                poll_slow(core, renderer);
                if core.quit_requested() {
                    slow_loop.quit();
                }
                gtk::glib::ControlFlow::Continue
            });

        let smoke_source = if smoke {
            let smoke_state = Rc::clone(&state);
            let smoke_finished = Rc::new(Cell::new(false));
            let callback_finished = Rc::clone(&smoke_finished);
            let started_at = Instant::now();
            let mut actions_sent = false;
            let source = gtk::glib::timeout_add_local(Duration::from_millis(1), move || {
                let mut state = smoke_state.borrow_mut();
                let Some((_core, renderer)) = state.as_mut() else {
                    callback_finished.set(true);
                    return gtk::glib::ControlFlow::Break;
                };
                if !actions_sent {
                    renderer.dispatch(AppAction::ShowSurface(crate::ui::SurfaceId::Mixer));
                    renderer.dispatch(AppAction::OpenTrayMenu);
                    actions_sent = true;
                }
                if started_at.elapsed() >= Duration::from_millis(200) {
                    renderer.dispatch(AppAction::Exit);
                    callback_finished.set(true);
                    gtk::glib::ControlFlow::Break
                } else {
                    gtk::glib::ControlFlow::Continue
                }
            });
            Some((source, smoke_finished))
        } else {
            None
        };

        main_loop.run();
        fast_source.remove();
        slow_source.remove();
        if let Some((source, smoke_finished)) = smoke_source {
            // The normal smoke path returns Break after dispatching Exit, so
            // GLib has already removed this source. Only remove it when the
            // main loop ended through an unexpected external quit.
            if !smoke_finished.get() {
                source.remove();
            }
        }
        let smoke_ok = if smoke {
            let state = state.borrow();
            state.as_ref().is_some_and(|(core, _)| {
                core.quit_requested() && core.state().is_visible(crate::ui::SurfaceId::Mixer)
            })
        } else {
            true
        };
        if let Some((_core, mut renderer)) = state.borrow_mut().take() {
            renderer.destroy();
        }
        if smoke_ok {
            Ok(())
        } else {
            Err("Linux host smoke actions did not complete through the production loop".into())
        }
    }

    struct SmokeAudio;

    impl AudioBackend for SmokeAudio {
        fn get_state(&self) -> Result<VolumeState, AudioError> {
            Ok(VolumeState {
                volume: 0.5,
                muted: false,
            })
        }

        fn set_volume(&self, _volume: f32) -> Result<(), AudioError> {
            Ok(())
        }

        fn toggle_mute(&self) -> Result<VolumeState, AudioError> {
            Ok(VolumeState {
                volume: 0.5,
                muted: true,
            })
        }

        fn set_mute(&self, _muted: bool) -> Result<(), AudioError> {
            Ok(())
        }
    }

    /// Run the real GTK host loop with deterministic in-process audio and
    /// renderer actions. This keeps the Xvfb smoke independent of PulseAudio
    /// while exercising the production timers, action queue, and teardown.
    pub fn run_smoke() -> Result<(), String> {
        crate::ui::platform::linux::renderer::ensure_gtk_initialized()?;
        run_loop(
            Some(Box::new(SmokeAudio)),
            None,
            None,
            None,
            config::load(),
            fallback_caps(),
            true,
        )
    }

    /// Run the Linux host to completion on GTK's process main thread.
    pub fn run() -> Result<(), String> {
        crate::ui::platform::linux::renderer::ensure_gtk_initialized()?;
        let config = config::load();
        let caps = detect_caps();

        let (audio, audio_error) = match default_backend() {
            Ok(audio) => (Some(audio), None),
            Err(error) => (None, Some(error.to_string())),
        };

        let (hotkeys, hotkey_error) = if is_x11() {
            match RdevHotkeys::new(config.modifier) {
                Ok(hotkeys) => (Some(Box::new(hotkeys) as Box<dyn HotkeySource>), None),
                Err(error) => (None, Some(format!("global hotkeys unavailable: {error}"))),
            }
        } else if is_wayland() {
            (
                None,
                Some("global hotkeys unavailable: current Linux provider requires X11".into()),
            )
        } else {
            (
                None,
                Some("global hotkeys unavailable: no X11 display".into()),
            )
        };

        run_loop(
            audio,
            audio_error,
            hotkeys,
            hotkey_error,
            config,
            caps,
            false,
        )
    }
}

/// Linux application entry point (GTK renderer).
#[cfg(all(target_os = "linux", feature = "gtk-renderer"))]
pub fn run() -> Result<(), String> {
    gtk_host::run()
}

/// Run the deterministic GTK/X11 host-loop smoke harness.
#[cfg(all(target_os = "linux", feature = "gtk-renderer"))]
pub fn run_smoke() -> Result<(), String> {
    gtk_host::run_smoke()
}
