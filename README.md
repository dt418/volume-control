# VolumeControl

A lightweight, **native** volume controller with global hotkeys, a system
tray, and an on-screen volume overlay — written in Rust.

Spiritual successor to [VolumePro](https://github.com/dt418/VolumeControl)
(AutoHotkey) — same interaction model, rebuilt as a cross-platform native
application: no webview, no Electron, no runtime dependencies beyond the OS.

## Features

- **Global hotkeys** (default `Ctrl+Alt`; `CapsLock` on Linux, which keeps
  GNOME/KDE's `Ctrl+Alt+↑/↓` workspace switching intact):
  - `Ctrl+Alt+↑ / ↓` — volume ±2%
  - `Ctrl+Alt+Shift+↑ / ↓` — volume ±10%
  - `Ctrl+Alt+M` — mute toggle
  - `Ctrl+Alt+R` — reset to 50%
  - `Ctrl+Alt+V` — open mixer *(planned)*
  - `Ctrl+Alt+Shift+M` — open tray menu (works even when the tray icon is
    hidden by Windows)
  - On macOS **both ⌘ (Command) and ⌃ (Control)** work as the primary
    modifier, so the `CtrlAlt` config matches `Ctrl+Alt` (⌃+⌥) while the
    macOS-native `⌘+⌥` spelling also works: `⌘/⌃+⌥+↑ / ↓` etc. Hold the
    combo to repeat the volume step continuously.
- **Media keys** (`Volume Up/Down/Mute`) keep the native Windows flyout — the
  app only stays in sync.
- **Overlay**: bottom-right popup with threshold-colored bar (grey / green /
  blue / orange-red) and the percentage; auto-hides after ~1.8 s;
  click-through.
- **System tray**: live volume label, mute toggle, reset, exit.
- **Start with the system**: native per-OS autostart — Windows
  `HKCU\...\CurrentVersion\Run`, macOS LaunchAgent, Linux XDG autostart.
  Toggle it in Settings or with `volumectl autostart on|off|status`.
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
  "modifier": "CtrlAlt",      // CtrlAlt | CapsLock | Alt | Ctrl (default CtrlAlt; CapsLock on Linux)
  "autostart": false,         // launch at login via the OS autostart mechanism
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
  (`volumectl get` / `set <0-100>` / `autostart <on|off|status>`); with them
  the native renderer builds:

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

## Running on macOS

The macOS release is a proper app bundle (`VolumeControl.app`) that runs a
global-hotkey host (no Dock icon, no windows yet — hotkeys + volume only):

1. Unzip the release archive and move `VolumeControl.app` to `/Applications`.
2. First launch is blocked by Gatekeeper because the app is ad-hoc signed.
   Right-click the app → **Open** → **Open**, or remove the quarantine flag
   in Terminal:

   ```bash
   xattr -dr com.apple.quarantine /Applications/VolumeControl.app
   ```

3. Grant **Accessibility** permission: **System Settings → Privacy & Security
   → Accessibility** and enable **VolumeControl** (the system prompts on
   first launch when possible). Without this permission macOS silently
   delivers no global key events, so the hotkeys appear dead.
4. Test with the default combo: hold **⌘+⌥+↑ / ↓** (or **⌃+⌥+↑ / ↓**) —
   volume changes immediately and keeps repeating every 50 ms while held.
   `⌘/⌃+⌥+M` mutes, `⌘/⌃+⌥+R` resets to 50%.
5. To quit: `pkill -x volumectl` (a menu-bar item is a follow-on task).

Running the raw binary from a terminal shows a startup banner with the config
path, the resolved modifier, and the permission state — useful for debugging.

## Platform status

| Feature                | Windows | macOS | Linux |
|------------------------|:-------:|:-----:|:-----:|
| Volume control         | ✅ WASAPI | ✅ CoreAudio | ✅ PulseAudio |
| Global hotkeys         | ✅ rdev | ✅ rdev (Accessibility permission) | ✅ rdev (X11) |
| Overlay                | ✅ | 🔜 | 🔜 |
| Mixer                  | ✅ | 🔜 | 🔜 |
| Settings window        | ✅ | 🔜 | 🔜 |
| System tray            | ✅ tray-icon | 🔜 | 🔜 |
| Live config reload     | ✅ | 🔜 | 🔜 |
| Adaptive UI renderer   | ✅ native Win32 | ✅ AppKit (surfaces + smoke-tested) | ✅ GTK4/libadwaita (surfaces, CI-tested under Xvfb) |

macOS and Linux currently run the cross-platform `rdev` hotkey host (with
their native audio backends); the full overlay/mixer/settings/tray surfaces
are Windows-first and land on the other platforms as follow-on work. The
AppKit and GTK4/libadwaita renderers already implement the same Signal Glass
surface contract as Windows behind the shared `NativeRenderer` bridge.

## CI and releases

GitHub Actions (`.github/workflows/`) verifies every push/PR:

- **Windows** — build, full test suite, release artifact validation.
- **macOS** — build and tests including the AppKit renderer smoke tests.
- **Ubuntu 24.04** — CLI fallback build/test, GTK4/libadwaita build and
  renderer smoke tests under Xvfb, and the Wayland layer-shell build.

Pushing a `v*` tag builds release binaries on all three platforms and
publishes a GitHub release with versioned archives and `SHA256SUMS.txt`
(`scripts/package.sh`).

You can also publish from the GitHub UI: open **Actions → Release → Run
workflow**, select the source branch, enter a version tag such as `v0.1.0`,
and run the workflow.

## Architecture

```
crates/volumectl/
├── src/
│   ├── audio/          AudioBackend trait (cross-platform)
│   ├── audio_windows   WASAPI via raw COM vtables (windows-sys)
│   ├── audio_macos     CoreAudio via the volumecontrol crate
│   ├── audio_linux     PulseAudio via the volumecontrol crate
│   ├── hotkeys/        HotkeyAction types
│   ├── hotkeys_rdev    global listener + hold-to-repeat (all platforms)
│   ├── autostart       launch-at-login (Windows Run, macOS LaunchAgent, Linux XDG)
│   ├── overlay         GDI-painted native popup (click-through, auto-hide)
│   ├── tray            tray-icon + muda context menu
│   ├── linux_app       GTK4 host (Linux, gtk-renderer feature)
│   ├── config          JSON config, mtime live reload
│   ├── core            shared volume/clamp/threshold logic (+ unit tests)
│   ├── ui/             shared adaptive UI contract (model, theme, capabilities,
│   │                   surface, settings) + platform renderer seams
│   └── cli             non-Windows CLI fallback
```

Windows-only modules are `#[cfg(target_os = "windows")]`-gated. The non-Windows
entry points run the `rdev` hotkey host with the native audio backend
(CoreAudio on macOS, PulseAudio on Linux); Linux additionally builds the GTK4
host with the `gtk-renderer` feature. The `ui` module defines the shared
renderer contract, and `ui/platform/macos` + `ui/platform/linux` are the
AppKit and GTK4/libadwaita renderer implementations.

## License

MIT
