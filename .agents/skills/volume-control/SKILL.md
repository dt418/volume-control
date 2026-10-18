---
name: volume-control-conventions
description: Development conventions and patterns for volume-control. Rust workspace, Tauri v2 desktop application, and conventional commits.
---

# Volume Control Conventions

> Generated from [dt418/volume-control](https://github.com/dt418/volume-control) and customized for ECC Tools.

## Overview

This skill teaches AI assistants the architecture, development patterns, and guardrails used in the `volume-control` repository.

## Tech Stack

- **Primary Language**: Rust 2021 (edition 2021)
- **Desktop Framework**: Tauri v2 (`src-tauri`)
- **Frontend**: React 19, TypeScript, Vite, Tailwind CSS v4 (`frontend/`)
- **Core Engine**: `crates/volumectl` (audio endpoint control, sessions, platform backends, CLI)
- **Platform Audio Backends**:
  - Windows: WASAPI, Win32 raw input / GDI / DirectX, Shell NotifyIcon
  - macOS: CoreAudio, Cocoa tray
  - Linux: PulseAudio / ALSA, X11 (input shape click-through) / Wayland (interactive overlay)

## When to Use This Skill

Activate this skill when:
- Making changes to this repository across Rust, Tauri, or frontend code
- Adding new features following established architectural patterns
- Writing unit, integration, or fixture tests that match project conventions
- Creating commits with proper Conventional Commit message format
- Ensuring compliance with repository guardrails and ECC reference-set baselines

## Commit Conventions

Follow Conventional Commits with concise, imperative subject lines (~50 characters):

### Prefixes
- `feat:` new feature or capability
- `fix:` bug fix or regression repair
- `docs:` documentation or reference-set updates
- `test:` test fixtures, unit tests, or integration tests
- `chore:` maintenance, dependencies, or tool configuration

### Rules
- Keep first line concise and descriptive
- Use imperative mood ("add feature" not "added feature")
- Substantive changes must always be accompanied by updates to `feature_list.json` and `claude-progress.md`

## Architecture & Code Style

### Rust Conventions (`crates/volumectl`, `src-tauri`)

| Element | Convention | Example |
|---------|------------|---------|
| Files & Modules | `snake_case` | `host_core.rs`, `window_manager.rs` |
| Functions & Methods | `snake_case` | `get_default_endpoint()`, `set_volume()` |
| Types & Traits | `PascalCase` | `VolumeController`, `PlatformHost` |
| Constants & Statics | `SCREAMING_SNAKE_CASE` | `DEFAULT_STEP_SIZE`, `WM_TRAYICON` |

```rust
// Preferred Rust module import style
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::host_core::AudioEndpoint;
```

### Frontend Conventions (`frontend/`)

| Element | Convention | Example |
|---------|------------|---------|
| Components | `PascalCase` | `MixerSurface.tsx`, `SettingsSurface.tsx` |
| Hooks & Utilities | `camelCase` | `useAudioSessions.ts`, `formatDecibels.ts` |
| Styling | Utility classes | Tailwind CSS v4 |

### Quality Gates & Guardrails

Run the full quality gate before committing or opening a PR:
1. `cargo fmt --all --check`
2. `git diff --check`
3. `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`
4. `cargo test --workspace --no-default-features`
5. `sh scripts/check-tauri-deadlock.sh` (when `src-tauri` code changed)
6. `sh scripts/check-records.sh --staged` (enforced by pre-commit hook)

Never weaken `-D warnings` or bypass checks with `--no-verify` without explicit approval.

### Safe Flow Rules

Destructive git commands (`push -f`, `reset --hard`, `clean -fd`, `checkout .`, `restore .`) are strictly blocked by `.claude/hooks/agent-safe-flow.sh`.
Use the canonical ship script (`scripts/ship.sh` or `scripts/ship.ps1`) for release flows.

---

*Customized for [ECC Tools](https://ecc.tools) and multi-agent harness compliance.*
