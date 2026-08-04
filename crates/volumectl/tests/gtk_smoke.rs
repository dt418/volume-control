//! GTK4/libadwaita renderer smoke test (spec §10.3 runtime evidence).
//!
//! `harness = false` + `required-features = ["gtk-renderer"]` in Cargo.toml:
//! the test binary's `main()` runs on the process main thread, where GTK is
//! legal. libtest would run the test body on a worker thread and GTK would
//! panic ("may only be used from the main thread").
//!
//! Requires a display session — CI runs it under `xvfb-run` on Ubuntu 24.04.
//! Without a display the binary skips with a message and exits 0, so headless
//! builds stay green.

fn main() {
    #[cfg(all(target_os = "linux", feature = "gtk-renderer"))]
    {
        run_smoke();
        eprintln!("gtk smoke OK");
    }
}

#[cfg(all(target_os = "linux", feature = "gtk-renderer"))]
fn run_smoke() {
    #[cfg(feature = "layer-shell")]
    use volumectl_lib::ui::platform::linux::renderer::layer_shell_available;
    use volumectl_lib::ui::platform::linux::renderer::{
        ensure_gtk_initialized, plan_surfaces, GtkPanel,
    };
    use volumectl_lib::ui::{
        tokens_for, AccentMode, AppState, ThemeMode, UiCapabilities, WorkArea,
    };

    fn caps() -> UiCapabilities {
        UiCapabilities {
            compositor: true,
            blur: true,
            high_contrast: false,
            reduced_motion: false,
            dpi_scale: 1.0,
            work_area: WorkArea::new(0, 0, 2560, 1400),
        }
    }

    fn state() -> AppState {
        AppState::from_audio(50, false, Some("Speakers".into()))
    }

    // GTK requires a display; skip cleanly headless (exit 0).
    if let Err(e) = ensure_gtk_initialized() {
        eprintln!("skipping GTK smoke test: no display (CI runs under xvfb-run): {e}");
        return;
    }

    // Material ladder + visibility flips on real windows.
    let tokens = tokens_for(ThemeMode::Dark, false, AccentMode::System, || Some(true));
    // Xvfb/X11: layer-shell is never available, so Blurred must land on the
    // Translucent window (glass requires a Wayland session).
    let plans = plan_surfaces(&state(), &tokens, &caps(), false);
    assert_eq!(plans.len(), 4);
    for plan in &plans {
        let mut panel = GtkPanel::new(plan.surface, false);
        panel.apply_plan(plan, &caps());
        assert_eq!(
            panel.material_kind(),
            plan.gtk_material,
            "panel must carry the planned material for {:?}",
            plan.surface
        );
        if plan.gtk_material.is_opaque() {
            assert_eq!(
                panel.opacity(),
                1.0,
                "opaque surface must be fully opaque ({:?})",
                plan.surface
            );
        }
        panel.set_visible(true);
        assert!(
            panel.is_visible(),
            "surface window must be visible after show ({:?})",
            plan.surface
        );
        panel.set_visible(false);
        assert!(
            !panel.is_visible(),
            "surface window must be hidden after hide ({:?})",
            plan.surface
        );
    }

    // High contrast forces every surface opaque.
    let mut hc_caps = caps();
    hc_caps.high_contrast = true;
    let hc_tokens = tokens_for(ThemeMode::Dark, true, AccentMode::System, || Some(true));
    for plan in &plan_surfaces(&state(), &hc_tokens, &hc_caps, true) {
        let mut panel = GtkPanel::new(plan.surface, true);
        panel.apply_plan(plan, &hc_caps);
        assert!(
            panel.material_kind().is_opaque(),
            "high contrast must force an opaque {:?}",
            plan.surface
        );
    }

    // Layer-shell is unreachable under X11/Xvfb: plans never go glass.
    #[cfg(feature = "layer-shell")]
    {
        assert!(
            !layer_shell_available(),
            "layer-shell must be absent under an X11 display"
        );
        let ls_plans = plan_surfaces(&state(), &tokens, &caps(), false);
        assert!(
            ls_plans.iter().all(|p| !p.gtk_material.is_glass()),
            "no Wayland glass without layer-shell support"
        );
    }
}
