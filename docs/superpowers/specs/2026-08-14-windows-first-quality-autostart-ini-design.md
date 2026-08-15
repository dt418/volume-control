# Windows-First Quality, Auto-Start, and INI Configuration

**Date:** 2026-08-14
**Status:** Approved design
**Scope:** Windows release quality first, followed by Linux and macOS parity

## Goal

Make the packaged VolumeControl release fail closed unless the complete
Windows UI, UX, functionality, stability, and performance checks pass. Add a
user-visible Windows auto-start setting and move the canonical configuration
from JSON to a human-readable INI file that can be edited directly through
Settings. Preserve safe migration and recovery for existing installations.

## Delivery order

1. Windows QA and release gate for the current Tauri surfaces and native host.
2. Windows auto-start command, Settings toggle, and integration evidence.
3. INI parser, migration/recovery, Settings persistence, and live reload.
4. Linux/macOS parser and feature-port checks; native auto-start adapters follow
   their platform conventions after the Windows contract is stable.

Each stage is independently testable. A later stage cannot weaken an earlier
release gate.

## Quality architecture

The test pyramid has three layers:

- Rust unit/integration tests cover hotkey mapping, AppCore routing, repeat
  state, config validation/migration, atomic persistence, and auto-start path
  or registry helpers.
- React component tests cover all Mixer, Settings, and Help controls, loading
  and error states, accessibility names, bounded shells, draft Save/Reset/Cancel
  behavior, and direct configuration editing.
- A real Windows smoke harness launches the release binary with an isolated
  config directory, exercises each surface and hotkey, captures Win32 evidence,
  checks process health, and cleans up only its child process.

The existing surface verifier remains the base. It gains a diagnostic-only
hotkey probe enabled by environment variables; normal launches do not write
probe data or change behavior. Probe records use monotonic timestamps for
registration, OS event receipt, AppCore handling, and state publication.

## Windows release gates

The release candidate must pass all of the following with exit code zero:

- `cargo fmt --all --check` and `git diff --check`.
- Workspace clippy with `--all-targets --no-default-features -- -D warnings`.
- Workspace Rust tests with `--no-default-features`.
- Full frontend Vitest suite and TypeScript/Vite build.
- Tauri release build with the pinned local CLI.
- Surface verifier for Mixer, Settings, and Help: successful bootstrap,
  expected client geometry, non-blank PrintWindow capture, and a live process
  after readiness.
- Hotkey smoke for all eight actions in a clean isolated configuration.
- Auto-start enable/read/disable/restore integration test.
- INI round-trip, JSON migration, malformed-file recovery, Settings editing,
  and live-reload tests.
- Records guard and repository self-tests.

The ship script is fail-closed: no artifact is presented as a release if any
gate fails. No gate may be bypassed with `--no-verify` or a warning suppression.

## Hotkey and performance contract

The eight Windows actions are:

1. Ctrl+Alt+Up — volume up.
2. Ctrl+Alt+Down — volume down.
3. Ctrl+Alt+Shift+Up — large volume up.
4. Ctrl+Alt+Shift+Down — large volume down.
5. Ctrl+Alt+M — toggle mute.
6. Ctrl+Alt+R — reset to 50%.
7. Ctrl+Alt+V — toggle Mixer.
8. Ctrl+Alt+Shift+M — open menu.

The fast host poll remains 20 ms and the repeat worker remains 50 ms. The
Windows probe records median and p95 first-action latency; the release budget
is p95 <= 75 ms, with repeat intervals expected near 50 ms. Registration must
be 8/8 in a clean test environment. If synthetic modifier injection cannot
reliably represent Shift on a particular desktop, the result is recorded as an
environment limitation and the physical-keyboard check remains required; it is
never reported as a false pass.

Surface readiness has a 2 second budget. A 30-60 second soak must not show a
process crash or repeated unhandled error. Working-set and CPU observations are
recorded for regression comparison; only deterministic latency, readiness, and
crash conditions are hard gates.

## Windows auto-start

The Windows adapter uses
`HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run` with the value
name `VolumeControl` and a correctly quoted executable path. Startup launches
the host in its normal tray/background state and does not open a webview
surface.

Tauri commands expose the typed contract:

- `get_autostart() -> { enabled, command }`
- `set_autostart(enabled) -> result`

Settings reads the actual registry state, provides a `Start with Windows`
toggle, displays write errors inline, and supports retry. Unit tests use an
in-memory or injected registry backend for parsing and quoting. A Windows
integration test writes the real user key, verifies read-back, and restores the
original value in `finally`. Linux and macOS initially expose the same command
shape with an explicit unsupported result; later adapters use XDG autostart
and launchd without changing the frontend contract.

## INI configuration and migration

`config.ini` is canonical and uses these sections:

```ini
[general]
volume_step=1
volume_step_large=10
overlay_duration_ms=1800
modifier=CtrlAlt

[appearance]
theme=System
material=Auto
motion=Full
accent=System

[feedback]
enabled=true
blocked_freq=400
blocked_duration_ms=80
limit_freq=600
limit_duration_ms=60

[color_thresholds]
green_up_to=40
blue_up_to=75
orange_up_to=100

[blacklist]
item.0=game.exe
item.1=chat.exe
```

The parser converts values into the existing typed config model and reuses the
current validation rules. Unknown sections/keys are ignored with a warning;
malformed values are errors with a safe fallback, never a panic. Writes use a
temporary file, flush/sync, and atomic rename. The existing mtime reload path
tracks the INI path.

When `config.ini` is absent and the legacy JSON file exists, the loader parses
and validates JSON, writes INI, and keeps the JSON as a migration backup. If an
INI later becomes invalid, a valid backup can be adopted and the Settings
surface shows a visible recovery warning. An invalid INI with no valid backup
uses validated defaults and reports the failure. Existing JSON is never deleted
automatically.

Settings remains the primary editing workflow. Each section updates a local
typed draft; Save writes one validated patch atomically, Reset restores the
committed draft, and Cancel closes without mutation. `config_path` returns the
INI path. Opening the file remains an advanced option, not a required user
workflow.

## Cross-platform follow-up

The parser, typed model, migration tests, and Settings contract run on all
platforms. After Windows evidence is green, Linux and macOS cross-target checks
compile the same code and run host-independent tests. Native auto-start is then
implemented with XDG desktop entries and launchd plists behind the same command
interface. Real desktop smoke evidence remains platform-specific and is never
claimed from a Windows host.

## Failure handling and evidence

Every smoke scenario writes JSON, PNG, and log artifacts beneath
`output/tauri-surface-evidence/` or a feature-specific evidence directory.
Temporary config directories and registry values are restored in `finally`.
Child-process cleanup is scoped by PID; unrelated VolumeControl instances are
never terminated. The final progress record includes exact commands, exit
codes, latency summaries, artifact paths, and any environment limitations.
