//! AppKit renderer smoke test (spec §10.2 runtime evidence).
//!
//! `harness = false` in Cargo.toml: the test binary's `main()` runs on the
//! process main thread, where AppKit is legal. libtest would run the test
//! body on a worker thread and `MainThreadMarker::new()` would panic instead.
//!
//! Exercised live on the macOS CI runner (macos-15, arm64). The binary
//! compiles everywhere (it is a Cargo `[[test]]` target on every platform)
//! but the body is gated to macOS, where the AppKit seam exists.

fn main() {
    #[cfg(target_os = "macos")]
    {
        run_smoke();
        eprintln!("appkit smoke OK");
    }
}

#[cfg(target_os = "macos")]
fn run_smoke() {
    use volumectl_lib::ui::platform::macos::renderer::{
        ensure_application, plan_surfaces, Panel, SurfacePlan,
    };
    use volumectl_lib::ui::{
        tokens_for, AccentMode, AppState, ThemeMode, UiCapabilities, WorkArea,
    };

    fn caps(high_contrast: bool) -> UiCapabilities {
        UiCapabilities {
            compositor: true,
            blur: true,
            high_contrast,
            reduced_motion: false,
            dpi_scale: 1.0,
            work_area: WorkArea::new(0, 0, 2560, 1400),
        }
    }

    fn plans(high_contrast: bool) -> Vec<SurfacePlan> {
        let tokens = tokens_for(ThemeMode::Dark, high_contrast, AccentMode::System, || {
            Some(true)
        });
        plan_surfaces(
            &AppState::from_audio(50, false, Some("Speakers".into())),
            &tokens,
            &caps(high_contrast),
        )
    }

    // One shared application instance for the process; AppKit requires it
    // before any window exists. Runs on the main thread by construction.
    ensure_application();

    // Material ladder + VoiceOver labels on real panels.
    let normal = plans(false);
    assert_eq!(normal.len(), 4, "all four surfaces must produce panels");
    for plan in &normal {
        let mut panel = Panel::new();
        panel.apply_plan(plan, &caps(false));
        assert!(
            panel.has_accessibility_label(),
            "every surface window must carry a VoiceOver label ({:?})",
            plan.surface
        );
        // The material ladder: opaque only when the resolved treatment is
        // Opaque; glass/translucent must leave the window translucent.
        assert_eq!(
            panel.is_opaque(),
            plan.appkit_material.is_opaque(),
            "opacity must match the resolved material for {:?}",
            plan.surface
        );
    }

    // High contrast forces every surface opaque.
    for plan in &plans(true) {
        let mut panel = Panel::new();
        panel.apply_plan(plan, &caps(true));
        assert!(
            panel.is_opaque(),
            "high contrast must force an opaque {:?}",
            plan.surface
        );
    }
}
