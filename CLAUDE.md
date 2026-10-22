# CLAUDE.md

Guidance for Claude Code. Read `AGENTS.md`, `GUARDRAILS.md` for gates/safe flow; `README.md` for build/platform/module detail.

## Project overview

VolumeControl: Rust 2021 workspace (`crates/volumectl`) + Tauri v2 host (`src-tauri`) + React/Vite frontend. Native volume controller: global hotkeys, WASAPI/CoreAudio/PulseAudio, host-owned confirmed state, tray/overlay/mixer/settings/help surfaces. Windows native Win32 host; macOS CoreAudio/AppKit; Linux PulseAudio + GTK4 or headless hotkey host. Platform code `#[cfg(target_os = "...")]`-gated. Keep `init.sh`; do not restore `init.ps1`.

## Commands

Bootstrap: `./init.sh`.

Windows builds: `scripts\win-build.bat build|run|test` (MSVC wrapper).

Canonical gate: `bash scripts/format-lint.sh` (`--skip-tests`, `--all-features` when appropriate); see `.agents/skills/format-lint/SKILL.md`. Full commands/platform packages: README "Building".

`src-tauri` surface changes: read `.agents/skills/tauri-deadlock-guard/SKILL.md`; run `bash scripts/check-tauri-deadlock.sh`. Project PreToolUse hook blocks direct pushes, destructive git, `--no-verify`; use `bash scripts/ship.sh --push`; delete merged branches only with `git branch -d` or `gh pr merge --delete-branch`.

## Architecture

See README "Architecture" and `.agents/skills/volume-control/SKILL.md` for module map. Key seams:

- `ui`: `AppState` confirmed state; `AppAction` renderer/hotkey intent; `NativeRenderer` publishes state; `HostHandle` routes actions. Renderers never mutate audio/config directly.
- `audio`: `AudioBackend` + Windows/macOS/Linux adapters.
- Hosts: `app.rs` (Win32), `macos_app.rs` (AppKit), `linux_host_core.rs` (display-free reducer), `linux_app.rs` (GTK).
- Config: typed INI under user config dir; legacy JSON migrated; mtime reload keeps active config on malformed input.
- Startup: Windows native host; macOS `macos_app`; Linux GTK with `gtk-renderer`, else headless hotkey host; explicit CLI args use `cli`.
- Linux global hotkeys currently require X11. On Wayland, GTK/audio/renderer remain available while hotkeys are reported as degraded. Missing layer-shell/PulseAudio at runtime is an environment skip, not a pass. The application must not install packages, invoke `sudo`, enable evdev grabbing, or require root/input-group privileges for Wayland hotkeys.

## Working rules

- One unfinished feature at a time; keep `feature_list.json` honest.
- brainstorm/spec -> plan -> execute -> verify -> review (`pre-push-review`) -> finish. No completion claim without fresh command output + exit-code evidence.
- Substantive changes update `feature_list.json` + `claude-progress.md` same change set. Preserve tests; record blocked/environment-dependent verification.
- Never bypass gates with `--no-verify` unless explicitly authorized. Before stop, leave clean restart path; record remaining unverified work.
