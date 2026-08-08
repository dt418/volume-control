# Linux Native Host Loop Design

**Date:** 2026-08-08
**Feature:** vol-011 follow-on — Linux GTK host wiring for X11 and Wayland
**Status:** Approved implementation scope

## Goal

Connect the existing Linux audio backend, X11 hotkey listener, shared UI action model, and GTK renderer through one main-thread-owned Linux host. The host must run on X11, remain usable with honest capability degradation on Wayland, and publish authoritative state without silently converting unavailable runtime capabilities into passes.

This is the next single feature slice after the macOS native host loop. It is deliberately a host-lifecycle slice, not a tray implementation or a visual redesign.

## Scope

Included:

- Linux-only host lifecycle in `linux_app.rs`.
- GTK/GLib default main-context ownership.
- Fast renderer/hotkey action polling and slower audio/config/publication polling.
- `AudioBackend`-based host ownership with injectable fake audio for host-core tests.
- X11 `rdev` hotkeys and listener-failure reporting.
- Explicit Wayland hotkey degradation without enabling evdev grabbing.
- Capability detection from the active GTK/GDK display where available.
- Authoritative `AppState` publication through `NativeRenderer::publish`.
- Config mtime reload with malformed-edit protection and hotkey modifier updates.
- Correct renderer-action routing through `HostHandle`.
- Correct layer-shell edge margins for nonzero and negative work-area origins.
- Noninteractive overlay and on-demand interactive mixer keyboard policy.
- Explicit renderer destruction and clean host shutdown.
- Harness-free X11/GTK host smoke coverage.
- Honest CI/runtime evidence for unavailable mandatory and optional dependencies.

Explicitly deferred:

- Linux tray, AppIndicator, and StatusNotifier integration.
- macOS or Windows host changes.
- Linux widget/content redesign beyond the minimum action bridge required for host verification.
- Linux blacklist/foreground-process gating.
- `rdev` evdev grabbing or a new Wayland global-hotkey provider.
- Fluent/WinUI 3, glass, and water-drop visual redesign.

`OpenTrayMenu` remains explicit, logs that Linux tray/menu integration is deferred, and never mutates audio or requests shutdown.

## Existing contracts

The implementation consumes these contracts without changing their public meaning:

- `crate::audio::AudioBackend` for read/set/toggle/set-mute operations.
- `crate::audio_linux::LinuxAudio` as the production Linux adapter.
- `crate::config::{load, load_existing, config_path}` and `Config` for appearance/hotkey settings.
- `crate::hotkeys::HotkeyAction` for listener events.
- `crate::hotkeys_rdev::RdevHotkeys` for X11 listener construction, action polling, modifier updates, status, and failure reporting.
- `crate::ui::AppAction` for renderer and hotkey intent.
- `crate::ui::AppState` for confirmed state and surface visibility.
- `crate::ui::{HostHandle, NativeRenderer, UiCapabilities, WorkArea, tokens_for}` for renderer ownership and publication.
- `crate::ui::platform::linux::renderer::LinuxRenderer` for GTK panels and layer-shell behavior.

The renderer never mutates audio or configuration directly. Renderer and hotkey intents enter the host action queue; the host applies and confirms them.

## Host ownership and thread model

`linux_app::run()` is called on the process main thread for the GTK native path. GTK initialization and every GTK/GDK/renderer operation remain on that thread.

The host owns:

- `Option<Box<dyn AudioBackend>>` for the current audio adapter.
- `Option<RdevHotkeys>` because Wayland does not provide the selected X11 hotkey provider.
- `LinuxRenderer`.
- Loaded `Config` and its last observed mtime.
- Confirmed `AppState`.
- `UiCapabilities` plus explicit degraded-capability status.
- Renderer-action channel receiver.
- Shutdown state and bounded audio-retry state.

The host uses GTK/GLib's default main context rather than creating an independent `MainContext`. This keeps GTK event sources, renderer operations, and host polling on one owner context. No async runtime or extra timer dependency is introduced.

## Startup sequence

`run()` performs these steps in order:

1. Initialize GTK through the existing renderer initialization seam.
2. Load the persisted configuration with the existing defaulting behavior.
3. Detect the active display/session and build a deterministic `UiCapabilities` snapshot.
4. Create the production audio adapter. If audio initialization fails, retain an explicit unavailable audio capability and continue if the renderer can initialize.
5. Determine the session backend. Construct `RdevHotkeys` only for the supported X11 provider; on Wayland retain `None` and report global hotkeys unavailable with the reason.
6. Create a `HostHandle` that sends `AppAction` values into a main-thread-owned channel.
7. Create `LinuxRenderer` with the handle and capability snapshot.
8. Initialize `AppState` from the first successful audio read, or from a safe unavailable-audio state when no backend is available.
9. Publish the initial state and theme tokens.
10. Install the fast and slow host sources on the default GLib context.
11. Run the GTK main loop until `Exit` or a fatal renderer/display error.
12. Destroy the renderer before returning.

A missing display or renderer creation failure is a native-host startup error and returns non-zero. It is not silently converted into a one-shot CLI success. Explicit CLI arguments continue to route to `cli::run()` unchanged.

Audio or hotkey unavailability after renderer initialization is a degraded host capability, not a renderer startup failure. The host remains alive so the user can see the error state and exit cleanly.

## Backend and capability policy

### X11

The selected Linux global-hotkey provider is `rdev` through the X11 display. The host creates `RdevHotkeys` only when the current display is X11 and the listener can start. It drains actions on the fast source and checks `listener_failure()` on every fast poll. A terminated listener is logged and marked unavailable; it does not force the GUI host to terminate.

X11 startup requires a usable `DISPLAY` and display access. An Xvfb smoke proves GTK/X11 initialization and renderer/host wiring only; it does not prove a physical desktop compositor or global hotkey permission.

### Wayland

The default Linux build does not claim Wayland global hotkeys. The host does not enable `rdev`'s `unstable_grab`/evdev path and does not request root or input-group privileges. On Wayland, the host reports a capability such as `global hotkeys unavailable: current Linux provider requires X11` and continues with GTK/audio/renderer functionality when those initialize.

A Wayland runtime pass requires an actual Wayland compositor. A build with Wayland headers or a GTK layer-shell library alone is not runtime evidence.

### Audio

`LinuxAudio` remains the production adapter. The host stores it behind `Box<dyn AudioBackend>` so action/state behavior is testable with a fake backend. If construction or an operation fails:

- log the concrete failure;
- keep the previous confirmed state rather than inventing a successful mutation;
- expose the unavailable/degraded audio capability;
- retry construction no more often than the slow source cadence with bounded retry state;
- continue processing renderer actions, hotkey status, config reload, and `Exit`.

A missing mandatory audio development package is a build failure for the supported Linux native profile, not a successful fallback. A running system with no PulseAudio/PipeWire-compatible server is an audio runtime-unavailable result, not an audio pass.

## Event loop and polling

The host installs two GLib timeout sources on the default context:

- **Fast source:** approximately 15 ms. Drain all queued renderer actions and hotkey actions, check listener failure, apply actions, and quit immediately when requested.
- **Slow source:** approximately 150 ms. Attempt bounded audio recovery, reload configuration when its mtime changes, read authoritative audio state, compute theme tokens, and publish once.

The action reducer is kept separate from source installation. Tests invoke the same `poll_fast`/`poll_slow` logic without GTK timers. Each audio mutation is followed by a later authoritative readback; the reducer never treats a requested value as confirmed before the backend accepts it.

If no action or state changes, the slow source may still publish the current authoritative snapshot at its normal cadence. The implementation must not busy-loop or create unbounded GLib sources.

## Action behavior

Audio actions:

- `SetVolumePercent { percent }`: clamp to 100 and call `set_volume`.
- `AdjustVolume { delta_percent }`: read current volume, apply signed percentage delta, clamp to 0–100, and call `set_volume`.
- `ToggleMute`: call `toggle_mute`.
- `SetMute { muted }`: call `set_mute`.
- `ResetVolume`: call `set_volume(0.5)`.

Surface actions:

- `ShowSurface`, `HideSurface`, and `ToggleSurface` update only host-owned visibility state.
- `OpenTrayMenu` logs `Linux tray/menu unavailable in this host` and does not terminate or mutate audio.
- `OpenConfigLocation` logs that native file-manager integration is deferred.

Appearance actions:

- `SetTheme`, `SetMaterial`, and `SetMotion` update the in-memory config and `AppState` immediately; the next slow publication resolves tokens.
- `ReloadConfig` reloads the persisted configuration and applies the new appearance and modifier.
- `ApplyConfig`, `CancelConfig`, `ResetConfig`, and blacklist actions log an explicit deferred message and leave the host alive.

Lifecycle:

- `Exit` requests shutdown.
- `OpenMixer` maps to `ToggleSurface(SurfaceId::Mixer)`.
- `OpenMenu` maps to `OpenTrayMenu` and remains non-fatal.

## Configuration reload

The host tracks `config::config_path()` mtime. On change:

- call `config::load_existing()`;
- replace the active config only when parsing/normalization succeeds;
- update appearance fields in `AppState`;
- call `RdevHotkeys::set_modifier()` when an X11 listener exists and the modifier changed;
- retain the old configuration on malformed or partially written edits;
- publish the next authoritative state.

## Renderer and geometry requirements

The Linux renderer continues to consume the shared placement model. Shared placement coordinates remain physical-pixel coordinates, and each platform converts them exactly once at its native renderer boundary. Any Linux logical-pixel conversion must not modify the shared geometry contract or the macOS AppKit conversion.

The macOS Retina regression for backing scale 2.0 must continue to pass unchanged: a physical 2880×1800 work area and physical surface must become the corresponding AppKit point rectangle after exactly one division by two.

For GTK layer-shell:

- use `work_area.right() - rect.right` for right margins;
- use `work_area.bottom() - rect.bottom` for bottom margins;
- do not use width/height-only arithmetic that assumes origin `(0, 0)`;
- overlay surfaces are noninteractive and do not claim keyboard focus;
- mixer surfaces become interactive only when shown/on demand.

Capability detection uses the active GDK display and monitor geometry where available. X11 may use EWMH work-area data when available and falls back to monitor geometry when it is not. Wayland uses output/monitor geometry and does not pretend that a universal taskbar work area exists. Missing optional accessibility queries retain deterministic fallback values and never make startup fail.

## Dependency classification

### Mandatory supported native profile

The supported GTK/audio Linux profile requires the GTK4 and libadwaita runtime/development packages, the PulseAudio development package used by `LinuxAudio`, and the X11 development/input libraries required by the selected `rdev` X11 provider. CI and documented installation instructions install these packages by default. The application does not invoke a package manager or `sudo` at runtime.

If a mandatory package is unavailable, verification reports a build failure or `SKIP — missing mandatory package: <package>` with the exact package name. It never records a passing native-host result.

### Optional capability

`gtk4-layer-shell` development/runtime support and a Wayland compositor implementing layer-shell are optional capabilities for this slice. Missing them may leave a plain GTK renderer path available, but layer-shell verification must be recorded as:

- `SKIP — libgtk-4-layer-shell-dev unavailable`, or
- `SKIP — no Wayland compositor available`,

with no `PASS` claim for that capability.

Xvfb is an X11 test display. It proves only the X11/GTK smoke scope and must not be used as evidence for Wayland or layer-shell runtime behavior.

## Verification evidence

Required automated evidence:

1. `cargo fmt --all --check` passes.
2. Windows and macOS builds/tests remain green with Linux host code gated out.
3. Linux no-feature cross-check remains green.
4. Linux GTK/layer-shell feature builds pass when mandatory and optional packages exist; missing optional packages are recorded as explicit skips.
5. Host-core tests cover fake-audio action/readback, audio failure retention, renderer channel delivery, config reload protection, modifier update, listener failure, shutdown, deferred tray action, X11/Wayland capability classification, and geometry origins.
6. The harness-free X11 smoke runs under Xvfb and proves GTK initialization, renderer construction, initial publication, representative renderer-to-host action delivery, surface state transition, and clean renderer destruction.
7. A Wayland smoke runs only on a real/nested compositor with the layer-shell dependency. It proves layer-shell mapping, overlay keyboard policy, interactive mixer policy, absolute-origin margins, honest hotkey degradation, and clean shutdown. If unavailable, records contain an exact `SKIP` reason rather than a pass.
8. Audio runtime evidence distinguishes a working PulseAudio/PipeWire-compatible server from an unavailable server.
9. `scripts/check-records.sh --staged` passes after records are staged with the implementation.
10. The full repository verification battery and three-domain pre-push review run before completion.

## Acceptance criteria

The Linux native path is accepted when a no-argument GTK build enters the shared host on X11 and on Wayland where GTK can initialize; X11 hotkeys are wired through the host; Wayland hotkeys degrade explicitly without evdev grabbing; audio/hotkey failures keep a live renderer host when possible; renderer actions reach host-owned state; configuration reload is safe; layer-shell geometry respects absolute work-area origins; overlay/mixer keyboard policy is correct; tray/menu remains explicit and non-fatal; renderer destruction is clean; and every unavailable mandatory/optional dependency or runtime environment is recorded honestly as a failure or reasoned skip.

The feature remains `vol-011: in_progress` until real macOS host smoke, Linux X11 evidence, Linux Wayland evidence or an honest environment skip, and the remaining platform verification are all recorded.
