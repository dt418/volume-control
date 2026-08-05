# VolumeControl

A lightweight, **native** volume controller with global hotkeys, a system
tray, and an on-screen volume overlay — written in Rust.

Spiritual successor to [VolumePro](https://github.com/dt418/VolumeControl)
(AutoHotkey) — same interaction model, rebuilt as a cross-platform native
application: no webview, no Electron, no runtime dependencies beyond the OS.

## Features

- **Global hotkeys** (default `Ctrl+Alt`):
  - `Ctrl+Alt+↑ / ↓` — volume ±2%
  - `Ctrl+Alt+Shift+↑ / ↓` — volume ±10%
  - `Ctrl+Alt+M` — mute toggle
  - `Ctrl+Alt+R` — reset to 50%
  - `Ctrl+Alt+V` — open mixer *(planned)*
  - `Ctrl+Alt+Shift+M` — open tray menu (works even when the tray icon is
    hidden by Windows)
- **Media keys** (`Volume Up/Down/Mute`) keep the native Windows flyout — the
  app only stays in sync.
- **Overlay**: bottom-right popup with threshold-colored bar (grey / green /
  blue / orange-red) and the percentage; auto-hides after ~1.8 s;
  click-through.
- **System tray**: live volume label, mute toggle, reset, exit.
- **Live config reload**: edit `config.json` and settings apply within
  ~150 ms — no restart needed.
- **External sync**: volume changed by media keys, other apps, or Bluetooth
  updates the tray label immediately.

## Configuration

On first run the app writes a default config to:

| OS      | Path |
|---------|------|
| Windows | `%APPDATA%\volume-control\config.json` |
| macOS   | `~/Library/Application Support/volume-control/config.json` |
| Linux   | `~/.config/volume-control/config.json` |

```jsonc
{
  "volume_step": 2,           // small step, percent (1-50)
  "volume_step_large": 10,    // Shift step, must be > volume_step
  "overlay_duration_ms": 1800, // overlay visibility (200-10000)
  "modifier": "CtrlAlt",      // CtrlAlt | CapsLock | Alt | Ctrl
  "blacklist": [],            // reserved for future use
  "color_thresholds": { "green_up_to": 40, "blue_up_to": 75, "orange_up_to": 100 }
}
```

## Building

Requirements: Rust (stable) + a C toolchain:

- **Windows**: MSVC Build Tools + Windows SDK. Build through
  `scripts\win-build.bat` (wraps `cargo` with the `vcvars64.bat` MSVC
  environment):

  ```bat
  scripts\win-build.bat build
  scripts\win-build.bat run
  scripts\win-build.bat test
  ```

- **macOS**: Rust (stable) + Xcode command-line tools:

  ```bash
  cargo build
  cargo test    # includes the AppKit renderer smoke tests
  ```

- **Ubuntu 24.04** (or Debian 12+): Rust (stable) + GTK4/libadwaita dev
  packages. Without them the binary builds as the CLI fallback
  (`volumectl get` / `set <0-100>`); with them the native renderer builds:

  ```bash
  sudo apt-get install libgtk-4-dev libadwaita-1-dev libpulse-dev xvfb
  cargo build                                    # CLI fallback
  cargo build --features gtk-renderer            # native GTK4 surfaces
  cargo build --features gtk-renderer,layer-shell  # + Wayland layer-shell overlay/mixer
  xvfb-run -a cargo test --features gtk-renderer # renderer smoke tests
  ```

  The Wayland layer-shell path also needs the GTK4 layer-shell development
  package (`libgtk4-layer-shell-dev`, when provided by the distribution);
  without it surfaces fall back to X11-compatible borderless windows.

## Platform status

| Feature                | Windows | macOS | Linux |
|------------------------|:-------:|:-----:|:-----:|
| Volume control         | ✅ WASAPI | 🔜 CoreAudio | 🔜 PulseAudio/PipeWire |
| Global hotkeys         | ✅ RegisterHotKey | 🔜 | 🔜 |
| Overlay                | ✅ | 🔜 | 🔜 |
| Mixer                  | ✅ | 🔜 | 🔜 |
| Settings window        | ✅ | 🔜 | 🔜 |
| System tray            | ✅ tray-icon | 🔜 | 🔜 |
| Live config            | ✅ | — | — |
| Adaptive UI renderer   | ✅ native Win32 | ✅ AppKit (surfaces + smoke-tested) | ✅ GTK4/libadwaita (surfaces, CI-tested under Xvfb) |

The macOS and Linux renderers implement the same Signal Glass surface
contract as Windows (placement, material ladder, motion policy, §11.2
accessibility vocabulary) behind the shared `NativeRenderer` bridge; their
host wiring (hotkeys, audio, tray) is follow-on work.

## CI and releases

GitHub Actions (`.github/workflows/`) verifies every push/PR:

- **Windows** — build, full test suite, release artifact validation.
- **macOS** — build and tests including the AppKit renderer smoke tests.
- **Ubuntu 24.04** — CLI fallback build/test, GTK4/libadwaita build and
  renderer smoke tests under Xvfb, and the Wayland layer-shell build.

Pushing a `v*` tag builds release binaries on all three platforms and
publishes a GitHub release with versioned archives and `SHA256SUMS.txt`
(`scripts/package.sh`).

## Architecture

```
crates/volumectl/
├── src/
│   ├── audio/          AudioBackend trait (cross-platform)
│   ├── audio_windows   WASAPI via raw COM vtables (windows-sys)
│   ├── hotkeys/        HotkeyAction types
│   ├── hotkeys_win32   Win32 RegisterHotKey + hidden-window message loop
│   ├── overlay         GDI-painted native popup (click-through, auto-hide)
│   ├── tray            tray-icon + muda context menu
│   ├── config          JSON config, mtime live reload
│   ├── core            shared volume/clamp/threshold logic (+ unit tests)
│   ├── ui/             shared adaptive UI contract (model, theme, capabilities,
│   │                   surface, settings) + platform renderer seams
│   └── cli             non-Windows CLI fallback
```

Windows-only modules are `#[cfg(target_os = "windows")]`-gated; the crate
still compiles on macOS/Linux (as the CLI utility) so non-Windows backends
can be added incrementally. The `ui` module defines the shared renderer
contract; `ui/platform/macos` and `ui/platform/linux` are compile-safe seams
(currently stubs) for the follow-on AppKit and GTK/libadwaita renderers.

## License

MIT
