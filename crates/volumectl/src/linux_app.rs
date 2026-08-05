//! Linux application host (GTK4 + PulseAudio).
//!
//! The Linux counterpart of the Windows `app` shell: binds the shared
//! [`crate::ui::NativeRenderer`] (the GTK renderer in
//! `ui::platform::linux`) to the Linux audio backend on the GTK main thread.
//! The host owns audio and configuration; the renderer only consumes
//! published [`AppState`] and emits [`AppAction`] values back through a
//! [`HostHandle`].
//!
//! The host runs a GTK main loop with a periodic poll (mirroring the Windows
//! 150 ms timer): it drains the action channel, applies each action to the
//! audio host, re-reads authoritative state, and republishes it to the
//! renderer. Compiled only on Linux with the `gtk-renderer` feature; without
//! a display session the caller falls back to the CLI.
//!
//! Tray / global-hotkey wiring is a separate follow-on; this host proves the
//! renderer + audio + main-loop foundation on a real Linux session.

#[cfg(all(target_os = "linux", feature = "gtk-renderer"))]
mod gtk_host {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use gtk4 as gtk;

    use crate::audio::AudioBackend;
    use crate::audio_linux::LinuxAudio;
    use crate::config::Config;
    use crate::ui::platform::linux::renderer::LinuxRenderer;
    use crate::ui::{
        tokens_for, AppAction, AppState, HostHandle, NativeRenderer, SurfaceVisibility,
        UiCapabilities, WorkArea,
    };

    /// Everything the GTK poll closure needs to drive the app on one thread.
    pub struct HostCtx {
        audio: LinuxAudio,
        renderer: LinuxRenderer,
        config: Config,
        state: AppState,
        caps: UiCapabilities,
        quit_requested: bool,
    }

    fn detect_caps() -> UiCapabilities {
        // On X11 (including Xvfb in CI) there is no compositing blur, so
        // surfaces resolve to the opaque fallback. DPI is 1.0 by default; a
        // real desktop would query the monitor geometry.
        UiCapabilities {
            compositor: false,
            blur: false,
            high_contrast: false,
            reduced_motion: false,
            dpi_scale: 1.0,
            work_area: WorkArea::new(0, 0, 1600, 900),
        }
    }

    fn refresh_from_audio(ctx: &mut HostCtx) {
        match ctx.audio.get_state() {
            Ok(vs) => {
                let mut state = ctx.state.clone();
                state.volume_percent = vs.percent();
                state.muted = vs.muted;
                state.device = None;
                ctx.state = state;
            }
            Err(e) => {
                log::warn!("audio readback failed: {e}");
            }
        }
        let tokens = tokens_for(
            ctx.config.appearance.theme,
            false,
            ctx.config.appearance.accent,
            || Some(ctx.caps.reduced_motion),
        );
        ctx.renderer.publish(&ctx.state, &tokens, &ctx.caps);
    }

    fn apply_action(ctx: &mut HostCtx, action: &AppAction) {
        use crate::ui::AppAction as A;
        match action {
            A::SetVolumePercent { percent } => {
                let _ = ctx.audio.set_volume((*percent).min(100) as f32 / 100.0);
            }
            A::AdjustVolume { delta_percent } => {
                let cur = ctx.audio.get_state().map(|s| s.volume).unwrap_or(0.0);
                let target = (cur * 100.0 + *delta_percent as f32).clamp(0.0, 100.0) / 100.0;
                let _ = ctx.audio.set_volume(target);
            }
            A::ToggleMute => {
                let _ = ctx.audio.toggle_mute();
            }
            A::SetMute { muted } => {
                let _ = ctx.audio.set_mute(*muted);
            }
            A::ResetVolume => {
                let _ = ctx.audio.set_volume(0.5);
            }
            A::ShowSurface(s) | A::HideSurface(s) => {
                let visible = matches!(action, A::ShowSurface(_));
                ctx.state.set_visibility(
                    *s,
                    if visible {
                        SurfaceVisibility::Visible
                    } else {
                        SurfaceVisibility::Hidden
                    },
                );
            }
            A::ToggleSurface(s) => {
                let vis = ctx.state.toggle(*s);
                log::info!("toggled {s:?} -> {vis:?}");
            }
            A::SetTheme(t) => ctx.config.appearance.theme = *t,
            A::SetMaterial(m) => ctx.config.appearance.material = *m,
            A::SetMotion(m) => ctx.config.appearance.motion = *m,
            A::Exit => {
                ctx.quit_requested = true;
            }
            _ => {}
        }
    }

    /// Run the Linux host to completion. Returns an error string on failure.
    pub fn run() -> Result<(), String> {
        crate::ui::platform::linux::renderer::ensure_gtk_initialized()?;
        let audio = LinuxAudio::new().map_err(|e| e.to_string())?;
        let config = crate::config::load();
        let caps = detect_caps();

        let (tx, rx) = channel::<AppAction>();
        let host = HostHandle::new(move |action| {
            let _ = tx.send(action);
        });

        let renderer = LinuxRenderer::create(host, caps).map_err(|e| e.to_string())?;

        let mut ctx = HostCtx {
            audio,
            renderer,
            config,
            state: AppState::default(),
            caps,
            quit_requested: false,
        };
        refresh_from_audio(&mut ctx);

        // Drive the host on a dedicated GLib main context. The default context
        // is acquired by the PulseAudio backend's mainloop thread, so the host
        // must not register its timer through `timeout_add_local`: that helper
        // always targets the default context. A local future is attached to
        // this context instead, keeping the GTK host alive until `Exit`.
        let main_context = gtk::glib::MainContext::new();
        let main_loop = gtk::glib::MainLoop::new(Some(&main_context), false);
        let main_loop_quit = main_loop.clone();
        let ctx_rc = Rc::new(RefCell::new(ctx));
        let rx_rc = Rc::new(rx);
        main_context
            .with_thread_default(|| {
                log::info!("linux host running on the GTK main loop");
                main_context.spawn_local(async move {
                    loop {
                        gtk::glib::timeout_future(Duration::from_millis(150)).await;
                        {
                            let mut c = ctx_rc.borrow_mut();
                            while let Ok(action) = rx_rc.try_recv() {
                                apply_action(&mut c, &action);
                            }
                        }
                        refresh_from_audio(&mut ctx_rc.borrow_mut());
                        if ctx_rc.borrow().quit_requested {
                            main_loop_quit.quit();
                            break;
                        }
                    }
                });
                main_loop.run();
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Linux application entry point (GTK renderer).
///
/// Initializes GTK; if no display session is available the error is returned
/// so the caller can fall back to the CLI.
#[cfg(all(target_os = "linux", feature = "gtk-renderer"))]
pub fn run() -> Result<(), String> {
    gtk_host::run()
}
