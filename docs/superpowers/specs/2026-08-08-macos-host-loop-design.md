# macOS Native Host Loop Design

**Date:** 2026-08-08
**Feature:** vol-011 follow-on — macOS AppKit host wiring
**Status:** Approved scope for implementation

## Goal

Connect the existing macOS CoreAudio backend, global hotkey listener, shared UI action model, and AppKit renderer through one main-thread-owned application host so launching `volumectl` without CLI arguments presents and updates the native macOS surfaces.

## Scope

This slice adds only the macOS native host loop. It does not add a tray/menu-bar implementation, refactor the Linux host, or change the Windows shell.

Included:

- macOS-only host module and entry-point wiring.
- AppKit application initialization and main-thread event processing.
- `MacAudio` ownership through the shared `AudioBackend` contract.
- `RdevHotkeys` action draining and listener-failure reporting.
- `HostHandle` channel delivery from `MacosRenderer` to the host.
- Authoritative audio state publication through `NativeRenderer::publish`.
- Shared UI actions for volume, mute, reset, surfaces, appearance, config reload, and exit.
- Clean renderer destruction and host shutdown.
- A harness-free macOS smoke test path suitable for the existing macOS CI runner.

Explicitly deferred:

- macOS tray/menu-bar integration. `AppAction::OpenTrayMenu` logs that the capability is unavailable and leaves the host running.
- Linux GTK host changes, X11 behavior changes, and Wayland layer-shell changes.
- Windows behavior and renderer changes.
- Fluent/WinUI 3, glass, and drop-water visual redesign. Those are a later UI feature with their own spec and plan.

## Existing contracts

The implementation must consume the existing contracts without changing their public meaning:

- `crate::audio::AudioBackend` for read/set/toggle/set-mute operations.
- `crate::config::load()` and `Config` for appearance and hotkey settings.
- `crate::hotkeys::HotkeyAction` for global listener events.
- `crate::hotkeys_rdev::RdevHotkeys` for listener construction, action polling, modifier updates, and listener failure reporting.
- `crate::ui::AppAction` for renderer and hotkey intent.
- `crate::ui::AppState` for confirmed state and surface visibility.
- `crate::ui::{HostHandle, NativeRenderer, UiCapabilities, WorkArea, tokens_for}` for renderer ownership and publication.
- `crate::ui::platform::macos::renderer::MacosRenderer` for AppKit panels.

## Host ownership and thread model

`macos_app` is compiled only for `target_os = "macos"`. Its `run()` function is called on the process main thread. The function must initialize AppKit before creating `MacosRenderer` or any AppKit panel.

The host owns:

- `MacAudio`.
- `RdevHotkeys`.
- `MacosRenderer`.
- Loaded `Config`.
- Confirmed `AppState`.
- A `UiCapabilities` snapshot.
- The renderer-action channel receiver.
- The shutdown flag.

The renderer is not permitted to mutate audio or configuration directly. Renderer actions enter the host channel through `HostHandle`; the host applies and confirms them.

The rdev listener and repeat worker remain on their existing background threads. The host drains their receiver on the AppKit main thread. The AppKit event loop and its manual `NSDate` deadline polling remain on the main thread so all `NSPanel` operations satisfy AppKit ownership rules.

## Startup sequence

`run()` performs these steps in order:

1. Ensure an `NSApplication` exists through the renderer's AppKit initialization seam.
2. Load the persisted configuration.
3. Create the native audio backend through `audio::default_backend()` or the concrete `MacAudio` adapter.
4. Create `RdevHotkeys` with the configured modifier.
5. Build a `HostHandle` whose callback sends `AppAction` values into a main-thread-owned channel.
6. Create `MacosRenderer` with the handle and detected capability snapshot.
7. Initialize `AppState` from the first audio read and copy the configured theme, material, and motion values.
8. Publish the initial state and theme tokens before entering the event loop.
9. Start the AppKit event loop and its periodic host poll.

If audio initialization, hotkey initialization, renderer creation, or AppKit setup fails, `run()` returns an error with the failing subsystem named. The existing `main.rs` error path prints the error and exits non-zero.

## Capability detection

The first host slice uses a deterministic capability snapshot appropriate to the existing renderer contract:

- `work_area`: the primary screen's visible frame converted to physical pixels when available; otherwise a safe 1600×900 fallback.
- `dpi_scale`: the primary screen backing scale factor when available; otherwise `1.0`. The renderer converts planned physical geometry back to AppKit points exactly once before calling `setFrame_display`.
- `compositor` and `blur`: true for a normal AppKit desktop surface.
- `high_contrast`: false in this first host slice because the optional accessibility appearance query is deliberately deferred; the renderer fallback remains deterministic.
- `reduced_motion`: false in this first host slice because the optional accessibility preference query is deliberately deferred; the renderer fallback remains deterministic.

Capability detection must not make startup fail merely because an optional accessibility query is unavailable. The renderer's material and motion fallback ladder remains authoritative.

## Event loop and polling

The host uses a main-thread-compatible AppKit event loop with a 150 ms `NSDate` deadline passed to `nextEventMatchingMask_untilDate_inMode_dequeue`, matching the Linux host and Windows timer cadence. Each poll executes in this order:

1. If `RdevHotkeys::listener_failure()` returns an error, log it and request shutdown with an error result.
2. Drain all queued hotkey actions and translate them using the current config step sizes.
3. Drain all queued renderer actions from the host channel.
4. Apply each action to the host state/backend.
5. Detect a changed config file mtime and reload configuration when necessary.
6. Re-read audio state.
7. Recompute theme tokens and call `renderer.publish(...)` once with the authoritative state.
8. If exit was requested, destroy the renderer and leave the event loop.

The implementation must avoid a busy loop. Each `NSDate` deadline waits approximately 150 ms for an AppKit event before the host poll continues; no extra timer dependency is required.

## Action behavior

Audio actions:

- `SetVolumePercent { percent }`: clamp to 100 and call `set_volume`.
- `AdjustVolume { delta_percent }`: read the current volume, apply the signed percentage delta, clamp to 0–100, and call `set_volume`.
- `ToggleMute`: call `toggle_mute`.
- `SetMute { muted }`: call `set_mute`.
- `ResetVolume`: call `set_volume(0.5)`.

Surface actions:

- `ShowSurface(surface)`, `HideSurface(surface)`, and `ToggleSurface(surface)` update only `AppState` visibility.
- `OpenTrayMenu` logs `macOS tray/menu unavailable in this host` and does not terminate or mutate audio.
- `OpenConfigLocation` logs that native file-manager integration is deferred; it does not crash.

Appearance actions:

- `SetTheme`, `SetMaterial`, and `SetMotion` update the in-memory config and `AppState` immediately, then are reflected by the next publication.
- `ReloadConfig` reloads the persisted config and applies the new appearance and hotkey modifier.
- Existing config mutation actions that are not backed by a macOS surface in this slice (`ApplyConfig`, `CancelConfig`, `ResetConfig`, blacklist actions) log an explicit deferred message and leave the host alive.

Lifecycle:

- `Exit` sets the shutdown flag.
- `OpenMixer` hotkey maps to `ToggleSurface(SurfaceId::Mixer)`.
- `OpenMenu` hotkey maps to `OpenTrayMenu`, which follows the deferred behavior above.

Errors from an individual audio mutation are logged, but the host continues and publishes the next readback. Startup errors and a terminated global listener return an error from `run()`.

## Configuration reload

The host tracks the mtime of `config::config_path()`. When it changes:

- Load and normalize through `config::load()`.
- Replace the in-memory config.
- Update `AppState` appearance fields.
- Call `RdevHotkeys::set_modifier()` if the modifier changed.
- Publish the next authoritative state.

A config reload failure must not erase the current working state. The existing `config::load()` defaulting behavior is used consistently with the other hosts.

## Entry-point behavior

`main.rs` keeps explicit CLI behavior unchanged:

- Any first argument continues to route to `cli::run()` on non-Windows targets.
- On macOS with no argument, route to `macos_app::run()`.
- Linux GTK, Linux non-GTK, and Windows routing remain unchanged.

## Verification

Required automated evidence for this slice:

1. `cargo fmt --all --check` passes.
2. Windows `cargo build` and `cargo test` remain green with the new module compiled out.
3. Linux no-feature cross-check remains green; GTK behavior is unchanged.
4. macOS `cargo check --target x86_64-apple-darwin -p volumectl --all-targets` passes with no warnings.
5. The harness-free macOS host smoke binary runs on the macOS CI runner and proves:
   - AppKit application initialization occurs on the main thread.
   - The host can construct `MacosRenderer`.
   - Initial state publication reaches the renderer path.
   - A representative renderer action reaches the host channel.
   - `OpenTrayMenu` is non-fatal and does not request shutdown.
   - `Exit` requests clean shutdown.
6. `scripts/check-records.sh --staged` passes after implementation and records are staged with the code.
7. The full repository verification battery required by `guardrail` is run before completion, with any unavailable real-desktop checks recorded honestly.

## Acceptance criteria

The feature is passing only when launching macOS without CLI arguments enters the AppKit host rather than the headless fallback, all supported renderer/hotkey actions use the shared host path, a terminated listener is surfaced, deferred menu actions are explicit and non-fatal, and fresh verification evidence is recorded in both `feature_list.json` and `claude-progress.md`.
