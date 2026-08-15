# VolumeControl

A lightweight, **native-first** volume controller with global hotkeys, a system
tray, an on-screen volume overlay, and Tauri v2 Mixer/Settings/Help surfaces —
written in Rust and React/TypeScript.

Spiritual successor to [VolumePro](https://github.com/dt418/VolumeControl)
(AutoHotkey) — same interaction model, rebuilt as a cross-platform native
application. The always-on host remains native; webviews are created lazily
only for the Mixer, Settings, and Help panels.

## Features

- **Global hotkeys** (default `Ctrl+Alt`, configurable per action in Settings):
  - `Ctrl+Alt+↑ / ↓` — volume ±1%
  - `Ctrl+Alt+Shift+↑ / ↓` — volume ±10%
  - `Ctrl+Alt+M` — mute toggle
  - `Ctrl+Alt+R` — reset to 50%
  - `Ctrl+Alt+V` — show or hide the mixer
  - `Ctrl+Alt+Shift+M` — open tray menu (works even when the tray icon is
    hidden by Windows)
  - On macOS **both ⌘ (Command) and ⌃ (Control)** work as the primary
    modifier, so the `CtrlAlt` config matches `Ctrl+Alt` (⌃+⌥) while the
    macOS-native `⌘+⌥` spelling also works: `⌘/⌃+⌥+↑ / ↓` etc. Hold the
     combo to repeat the volume step continuously.
- **Shortcut editor**: press `Record` and type a modifier + key combination;
  `Clear` disables that action, and duplicate/conflicting bindings are reported
  before Save.
- **Mixer surface**: system output controls, threshold-aware signal rail,
  per-app sessions on Windows, search, optimistic controls, and retryable
  backend recovery.
- **Settings and Help surfaces**: settings are edited in a draft and committed
  atomically; Help reflects the recorded shortcuts and registration status.
- **Media keys** (`Volume Up/Down/Mute`) keep the native Windows flyout — the
  app only stays in sync.
- **Overlay**: bottom-right popup with threshold-colored bar (grey / green /
  blue / orange-red) and the percentage; auto-hides after ~1.8 s;
  click-through.
- **System tray**: live volume label, mute toggle, reset, exit.
- **Settings-first configuration**: common options, appearance, blacklist,
  feedback, storage, and shortcuts can be changed directly in Settings. The
  JSON file remains available for automation and advanced edits.
- **Live config reload**: edits to `config.json` are detected and applied
  within ~150 ms — no restart needed.
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
  "volume_step": 1,           // small step, percent (1-50)
  "volume_step_large": 10,    // Shift step, must be > volume_step
  "overlay_duration_ms": 1800, // overlay visibility (200-10000)
  "modifier": "CtrlAlt",      // CtrlAlt | CapsLock | Alt | Ctrl
  "blacklist": [],            // executable names excluded from hotkeys
  "color_thresholds": { "green_up_to": 40, "blue_up_to": 75, "orange_up_to": 100 },
  "hotkeys": {
    "volume_up": "Ctrl+Alt+ArrowUp",
    "volume_down": "Ctrl+Alt+ArrowDown",
    "volume_up_large": "Ctrl+Alt+Shift+ArrowUp",
    "volume_down_large": "Ctrl+Alt+Shift+ArrowDown",
    "toggle_mute": "Ctrl+Alt+KeyM",
    "reset_50": "Ctrl+Alt+KeyR",
    "open_mixer": "Ctrl+Alt+KeyV",
    "open_menu": "Ctrl+Alt+Shift+KeyM"
  }
}
```

An empty value in `hotkeys` disables only that action. Existing configs without
the `hotkeys` object migrate to the selected modifier preset automatically.

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

- **Tauri webview surfaces** (all platforms): Node.js 22+ and the frontend
  dependencies:

  ```bash
  npm ci --prefix frontend
  npm --prefix frontend run build
  npm --prefix frontend test
  ```

  The isolated desktop E2E package lives in `e2e/tauri`; install it with
  `npm ci --prefix e2e/tauri`. Run the Windows release-gate matrix with
  `scripts\verify-tauri-e2e.ps1 -Surface all` or the Linux/macOS shell wrapper.
  Tauri's configured dev/build hooks use the CLI's object form with
  `cwd: ../frontend`, so the frontend command is independent of the directory
  from which the Tauri CLI is invoked and cannot search for a missing root
  `package.json`.

  Set `E2E_DRIVER_PROVIDER=embedded` (or a preflighted `tauri-driver`) explicitly
  before WDIO. Verify the provider contract with
  `node e2e/tauri/test-provider.mjs --provider embedded --platform windows`.
  WDIO shortcut cards prove only UI/configuration; Linux/Xvfb and hosted macOS
  do not prove native shortcut delivery. On a real Windows desktop, run
  `pwsh -NoProfile -File scripts/verify-hotkey-latency.ps1 -Release -Iterations 10 -OutputRoot output/manual/hotkey-latency`.
  This sends the configured `open_mixer` shortcut with `keybd_event` and writes
  the real OS integration report to
  `output/manual/hotkey-latency/hotkey-latency.json` and `.txt`.

  For the complete release candidate procedure and evidence classifications,
  see the [cross-platform release checklist](docs/testing/cross-platform-release-checklist.md).

## Running on macOS

The macOS release is a proper app bundle (`VolumeControl.app`) that runs the
global-hotkey/audio host and can open the Tauri Settings, Help, and Mixer
surfaces:

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
5. To quit: `pkill -x VolumeControl` (a menu-bar item is a follow-on task).

Running the raw binary from a terminal shows a startup banner with the config
path, the resolved modifier, and the permission state — useful for debugging.

## Platform status

| Feature                | Windows | macOS | Linux |
|------------------------|:-------:|:-----:|:-----:|
| Volume control         | ✅ WASAPI | ✅ CoreAudio | ✅ PulseAudio |
| Global hotkeys         | ✅ global-hotkey | ✅ global-hotkey | ✅ global-hotkey (X11) |
| Overlay                | ✅ native Win32 | 🔜 host integration | 🔜 host integration |
| Mixer                  | ✅ Tauri + WASAPI sessions | ✅ Tauri surface / 🔜 per-app audio | ✅ Tauri surface / 🔜 per-app audio |
| Settings window        | ✅ Tauri | ✅ Tauri | ✅ Tauri |
| System tray            | ✅ tray-icon | 🔜 | 🔜 |
| Live config reload     | ✅ | ✅ core | ✅ core |
| Adaptive UI renderer   | ✅ native Win32 | ✅ AppKit (surfaces + smoke-tested) | ✅ GTK4/libadwaita (surfaces, CI-tested under Xvfb) |

macOS and Linux run the cross-platform `global-hotkey` host with their native
audio backends. Tauri Settings/Help/Mixer surfaces are cross-platform; per-app
session enumeration and the native overlay/tray host remain Windows-first.
The AppKit and GTK4/libadwaita renderers implement the same Signal Glass
surface contract behind the shared `NativeRenderer` bridge.

The platform table describes implemented surfaces, not proof that every hosted
runner can exercise native OS integration. WDIO proves surface/UI/configuration
and IPC behavior. Windows shortcut delivery, tray behavior, hardware audio,
TCC/Accessibility, menu-bar behavior, Wayland compositor behavior, and
multi-monitor geometry require the manual evidence described in the
[cross-platform release checklist](docs/testing/cross-platform-release-checklist.md).

## CI and releases

GitHub Actions (`.github/workflows/`) runs deterministic checks and desktop
validation according to the event schedule:

- **Windows** — build, full test suite, and the Windows WDIO release gate.
- **macOS** — build and AppKit renderer smoke tests in full merge/release
  validation.
- **Ubuntu 24.04** — CLI fallback build/test, GTK4/libadwaita smoke tests under
  Xvfb, and optional layer-shell compilation in full validation.
- **Desktop E2E** — isolated WebdriverIO/Tauri surface tests for Mixer,
  Settings, Help, recovery, owned windows, and the runtime bridge. Debug-only
  Tauri Pilot scenarios are retained for exploratory replay; WDIO is the
  release gate. Hosted headless jobs do not claim native hotkey, tray,
  hardware-audio, Wayland-compositor, or multi-monitor coverage.

Pushing a `v*` tag first validates the tag, then runs the reusable SHA-bound
Windows/macOS/Ubuntu desktop matrix. The publish job verifies each platform's
metadata, package checksum, and package contents before promoting versioned
archives and `SHA256SUMS.txt`; it does not rebuild an unvalidated binary. E2E
JUnit, manifest, and platform-log evidence is produced and reviewed separately
as part of the validation artifact; the publish verifier does not re-verify
those files.

The current macOS package is ad-hoc signed for validation and local inspection,
not public distribution. Public macOS distribution requires a future protected
Developer ID and notarization workflow (`notarytool`, stapling, `spctl`, and
temporary keychain cleanup). No signing credentials are stored in the
repository. See the [cross-platform release checklist](docs/testing/cross-platform-release-checklist.md)
for the exact inspection commands and signing boundary.

You can also publish from the GitHub UI: open **Actions → Release → Run
workflow**, select the source branch, enter a version tag such as `v0.1.0`,
and run the workflow.

## Architecture

```
frontend/                    React + TypeScript + Vite webview surfaces
e2e/tauri/                   isolated WebdriverIO/Tauri E2E package
src-tauri/                   Tauri v2 host, commands, and window manager
crates/volumectl/
├── src/
│   ├── audio/          AudioBackend trait (cross-platform)
│   ├── audio_windows   WASAPI via raw COM vtables (windows-sys)
│   ├── audio_macos     CoreAudio via the volumecontrol crate
│   ├── audio_linux     PulseAudio via the volumecontrol crate
│   ├── hotkeys/        HotkeyAction types
│   ├── hotkeys_global   global listener + hold-to-repeat (all platforms)
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
entry points run the `global-hotkey` hotkey host with the native audio backend
(CoreAudio on macOS, PulseAudio on Linux); Linux additionally builds the GTK4
host with the `gtk-renderer` feature. The `ui` module defines the shared
renderer contract, and `ui/platform/macos` + `ui/platform/linux` are the
AppKit and GTK4/libadwaita renderer implementations.

## License

MIT
