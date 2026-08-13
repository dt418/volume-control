# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with this code repository.

## Project overview

VolumeControl is a Rust 2021 Cargo workspace (`rust-version = 1.82`) containing the
`crates/volumectl` library and `volumectl` binary. It is a native volume controller
with global hotkeys, native audio backends, host-owned confirmed state, and platform
renderers. Windows has the complete Win32 tray/overlay host. macOS has CoreAudio,
global-hotkey, and an AppKit host/renderer. Linux has PulseAudio, global-hotkey, a platform-neutral
host reducer, and an optional GTK4/libadwaita host/renderer. Non-Windows tray/menu,
full surface interaction, settings persistence actions, and blacklist actions remain
follow-on work.

Use `#[cfg(target_os = "...")]` for platform-specific code. Keep `init.sh` unchanged;
the abandoned `init.ps1` bootstrap task must not be restored.

## Commands

Run the baseline bootstrap from the repository root:

```bash
./init.sh
```

Windows builds must use the MSVC environment wrapper:

```bat
scripts\win-build.bat build
scripts\win-build.bat run
scripts\win-build.bat test
```

Common Rust checks and tests:

```bash
cargo fmt --all --check
git diff --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
cargo test -p volumectl <filter>
cargo test -p volumectl --test linux_host_core
cargo test -p volumectl --test macos_host_smoke
```

The canonical manifest-driven gate is authoritative for local and CI quality checks:

```bash
bash scripts/format-lint.sh
bash scripts/format-lint.sh --skip-tests
bash scripts/format-lint.sh --all-features
bash scripts/check-records.sh --staged
bash scripts/check-records.sh --branch origin/master
bash scripts/test-check-records.sh
bash scripts/test-format-lint.sh
bash scripts/test-ship.sh
bash scripts/ship.sh --dry-run
```

The pre-commit hook is intentionally lightweight; the full workspace test suite is
run by the canonical gate and CI. `scripts/format-lint-steps.json` is the single
source of truth for gate steps and forbidden paths. `AGENTS.md` and `GUARDRAILS.md`
define the detailed enforcement and shipping rules. Use `rtk` prefixes according to
the global Claude Code instructions when executing commands.

On macOS, the normal native checks are:

```bash
cargo build
cargo test
```

Ubuntu/Debian GTK development needs the packages used by CI, including the X11
libraries required by the global hotkey and X11 backends:

```bash
sudo apt-get install libgtk-4-dev libadwaita-1-dev libpulse-dev \
  libx11-dev libxi-dev libxtst-dev xvfb
```

GTK host and X11 smoke checks:

```bash
cargo build --features gtk-renderer
xvfb-run -a cargo test --features gtk-renderer
xvfb-run -a cargo test -p volumectl \
  --features gtk-renderer \
  --test linux_host_smoke -- --nocapture
```

Wayland layer-shell is optional and depends on the distribution providing
`libgtk4-layer-shell-dev`/`gtk4-layer-shell-0.pc`:

```bash
cargo build --features gtk-renderer,layer-shell
```

Missing optional layer-shell libraries or a Wayland compositor are environment
skips, not passes. Xvfb proves only X11/GTK behavior. Missing PulseAudio/PipeWire
runtime is audio-unavailable evidence, not a successful audio verification. The
application must not install packages, invoke `sudo`, enable evdev grabbing, or
require root/input-group privileges for Wayland hotkeys.

## Startup behavior

- Windows starts the native Win32 host.
- macOS with no arguments starts `macos_app`; explicit `get`, `set <0-100>`, and
  `mute` arguments use `cli`.
- Linux with `gtk-renderer` and no arguments starts `linux_app`; explicit CLI
  arguments still use `cli`.
- Linux without GTK and no arguments starts the headless `hotkeys_global` host;
  explicit CLI arguments still use `cli`.
- Linux global hotkeys currently require X11. Wayland keeps GTK/audio/renderer
  operation available while recording degraded hotkey capability.

## Architecture

The shared host boundary is in `ui`: `AppState` is the confirmed state published by
hosts, `AppAction` carries renderer/hotkey intent, `NativeRenderer` publishes state
and accepts actions, and `HostHandle` routes actions without letting renderers mutate
audio or configuration directly. Shared model, theme, capability, placement, and
surface contracts live beside these interfaces.

Audio uses `crate::audio::AudioBackend` with target adapters in `audio_windows.rs`
(WASAPI), `audio_macos.rs` (CoreAudio), and `audio_linux.rs` (PulseAudio). Hotkey
actions live in `crate::hotkeys`; the cross-platform listener and hold-repeat logic
are in `hotkeys_global.rs`. Windows native `RegisterHotKey` wiring is owned by
`app.rs`.

Platform hosts own event loops, backend state, configuration reload, and confirmed
state publication:

- `app.rs` owns the Windows message loop, tray, overlay, mixer, settings, and help.
- `macos_app.rs` owns the AppKit-main-thread loop, CoreAudio, global-hotkey actions, mtime
  config reload, `MacosRenderer`, and explicit renderer teardown. Retina geometry
  converts physical pixels to AppKit points exactly once using the backing scale.
- `linux_host_core.rs` is the display-free reducer and test seam for audio, hotkeys,
  actions, configuration reload, retries, and degraded state.
- `linux_app.rs` owns GTK initialization, display capability detection, the GLib fast
  and slow polls, renderer/action routing, recovery, and teardown. The GTK renderer
  in `ui/platform/linux` uses Wayland layer-shell when available and X11/plain-window
  fallbacks otherwise. `ui/platform/macos` contains the AppKit renderer.

Configuration is JSON under the platform user config directory. Hosts use safe mtime
reload: a valid parsed and normalized config replaces active state; malformed input
leaves the current configuration intact. Audio failures preserve the last confirmed
state and use bounded recovery rather than fabricating a zero volume.

## Working rules

- Work on one unfinished feature at a time; keep `feature_list.json` honest.
- Follow brainstorm/spec -> plan -> execute -> verify -> review (`pre-push-review`)
  -> finish. Do not claim completion without fresh command output and exit-code
  evidence.
- Substantive changes to code, scripts, CI, hooks, skills, or this file update both
  `feature_list.json` and `claude-progress.md` in the same change set.
- Preserve tests and add records for blocked or environment-dependent verification.
- Never bypass repository gates with `--no-verify` unless explicitly authorized.
- Before stopping, leave a clean restart path and record remaining unverified work.
