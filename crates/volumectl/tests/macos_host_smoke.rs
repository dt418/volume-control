//! Harness-free macOS host smoke test.
//!
//! AppKit requires its objects to be created on the process main thread. A
//! normal libtest function runs on a worker thread, so this binary keeps the
//! assertions in `main()` just like `appkit_smoke.rs`.

fn main() {
    #[cfg(target_os = "macos")]
    run_smoke();
}

#[cfg(target_os = "macos")]
fn run_smoke() {
    use std::sync::mpsc::channel;

    use volumectl_lib::macos_app::{action_requests_shutdown, deferred_tray_menu_is_non_fatal};
    use volumectl_lib::ui::platform::macos::renderer::{ensure_application, MacosRenderer};
    use volumectl_lib::ui::{
        tokens_for, AppAction, AppState, HostHandle, NativeRenderer, ThemeMode, UiCapabilities,
        WorkArea,
    };

    ensure_application();

    let caps = UiCapabilities {
        compositor: true,
        blur: true,
        high_contrast: false,
        reduced_motion: false,
        dpi_scale: 1.0,
        work_area: WorkArea::new(0, 0, 2560, 1400),
    };
    let (tx, rx) = channel();
    let host = HostHandle::new(move |action| {
        tx.send(action).expect("host channel remains open");
    });
    let mut renderer = MacosRenderer::create(host, caps).expect("macOS renderer creates");

    renderer.dispatch(AppAction::ToggleMute);
    assert_eq!(
        rx.recv().expect("renderer action reaches host"),
        AppAction::ToggleMute
    );
    assert!(
        deferred_tray_menu_is_non_fatal(),
        "deferred tray menu must not request host shutdown"
    );
    assert!(
        !action_requests_shutdown(&AppAction::OpenTrayMenu),
        "OpenTrayMenu must leave the host running"
    );
    assert!(
        action_requests_shutdown(&AppAction::Exit),
        "Exit must request host shutdown"
    );

    let state = AppState::from_audio(50, false, None);
    let tokens = tokens_for(ThemeMode::Dark, false, Default::default(), || Some(true));
    renderer.publish(&state, &tokens, &caps);
    renderer.destroy();

    eprintln!("macos host smoke OK");
}
