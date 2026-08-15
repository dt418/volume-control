# Cross-Platform Testing and Release Safety Design

**Date:** 2026-08-15
**Status:** Draft for review
**Approved direction:** Balanced (Plan B)

## Objective

Increase defensible release confidence for Windows, Linux, and macOS while
preserving the existing VolumeControl test and CI architecture. The design
prioritizes deterministic evidence, fail-closed release gates, and useful
manual coverage where hosted runners cannot reproduce OS integration behavior.

This design also resolves the orchestration review findings so the documented
coordinator workflow is reproducible from a clean checkout.

## Scope

### In scope

- Reconcile the existing WDIO/Pilot implementation with the older unchecked
  implementation plan.
- Make the Codex role configuration reproducible and permit the documented
  coordinator handoffs.
- Define a fail-closed E2E evidence contract.
- Define Windows-first PR validation and full merge/release validation.
- Define the exact confidence boundaries for Windows CI/manual testing, Linux
  Xvfb/WSLg, and hosted macOS runners.
- Prevent release tags from bypassing required validation.
- Define the future production macOS signing/notarization boundary.

### Explicit non-goals

- Replacing Tauri, WebdriverIO, Tauri Pilot, or GitHub Actions.
- Rewriting platform abstractions or the frontend solely for test coverage.
- Pixel-perfect or screenshot-diff testing without a demonstrated regression
  signal.
- Treating Tauri Pilot as a release dependency.
- Adding paid cloud Macs, self-hosted runners, or Docker GUI infrastructure.
- Automatically producing universal macOS binaries before the supported
  architecture is explicitly chosen.

## Current architecture and test seams

The workspace contains `crates/volumectl` and `src-tauri`. The shared crate
owns AppCore, configuration, audio traits/backends, hotkey handling, and
platform seams. The Tauri host owns IPC commands, WindowManager, lifecycle,
debug plugin registration, and the webview surfaces. The frontend provides
Mixer, Settings, Help, recovery, and shortcut UI.

Platform-specific seams are:

- Windows: WASAPI, native overlay, tray, wheel bridge, and Windows host loop.
- Linux: PulseAudio/PipeWire seam, X11 global-hotkey path, GTK4/libadwaita
  renderer, and optional gtk4-layer-shell build.
- macOS: CoreAudio, cross-platform global-hotkey host, and AppKit renderer
  smoke tests.

The existing `e2e/tauri` package already contains WDIO specs, debug capability
preparation, isolated config directories, artifact helpers, recovery/window
coverage, and a runtime bridge check. `e2e/pilot` contains local debug replay
scenarios. The implementation plan must describe only the missing delta.

## Orchestration integrity

The repository-local `.codex/config.toml` registers `planner`, `implementer`,
`release_reviewer`, and `coordinator`, but the four referenced profiles and
`ORCHESTRATOR.md` are currently untracked. A clean checkout therefore cannot
reproduce the configured workflow.

The approved remediation is:

1. Track the four referenced role profiles and `.codex/ORCHESTRATOR.md`.
2. Change `[agents].max_depth` from `1` to `2`, retaining `max_threads = 4`.
   This permits `root → coordinator → planner/implementer/release_reviewer`
   without opening unbounded fan-out.
3. Update `.codex/AGENTS.md` so its role list matches the runtime config.
4. Add a read-only clean-checkout contract that parses the TOML and verifies
   every configured profile exists.
5. Keep model routing advisory until a restarted runtime confirms the current
   catalog IDs. Do not silently claim Terra/Luna routing is active when profile
   fields are comments.

## Validation tiers

### Tier 1: deterministic checks

- Rust formatting, clippy, workspace tests, and record/diff guards.
- Frontend build and tests.
- Pure AppCore/config/hotkey/renderer contract tests.
- E2E helper and production-exclusion contract tests.
- Config/profile parse and clean-checkout reproducibility test.

These are cheap and remain required for every PR.

### Tier 2: desktop runtime smoke

- WDIO against a debug Tauri binary with the WDIO plugin enabled.
- Surface readiness, IPC roundtrip, recovery state, owned windows, and
  meaningful Mixer/Settings/Help interactions.
- One worker per run, isolated config directory, PID-scoped cleanup, and
  condition-based waits.
- JUnit output, screenshots/accessibility snapshots on failure, frontend and
  backend error logs, timing report, and an artifact manifest.

The runtime gate must fail when required evidence is missing, when cleanup
fails, or when uncaught frontend/backend errors are recorded. A successful run
must never rely on `if-no-files-found: warn`.

The release binary is not a WDIO target. Production validation uses a separate
binary path for startup/packaging checks and for asserting that debug plugins
and capabilities are excluded.

### Tier 3: OS integration

OS integration is split between hosted CI and manual checks. Hosted CI must
state exactly what it proves; manual checklists must cover behavior that
requires a real desktop session or user permission.

## Platform strategy

### Windows-first

Every PR runs the deterministic checks and Windows WDIO gate. Windows CI also
builds and validates the release host. A Windows manual release checklist is
required for:

- real global shortcut registration, conflict/disabled states, and latency;
- tray menu and overlay focus/positioning;
- WASAPI output and per-application session behavior;
- media keys and hold-to-repeat behavior;
- multi-monitor/scaling behavior;
- clean startup/shutdown and autostart behavior when enabled.

The checklist records OS version, display topology, audio endpoint, shortcut,
measured latency, and artifact/build SHA.

### Linux CI and WSLg

Ubuntu CI under Xvfb proves deterministic Tauri/DOM/IPC behavior, GTK smoke
tests, CLI/core tests, and optional layer-shell compilation when the system
library exists. It does not claim real Wayland compositor, tray, audio device,
global-hotkey, or multi-monitor confidence.

WSLg is an on-demand manual environment for GTK interaction, window focus and
positioning, Wayland/X11 behavior, and available audio routing. WSLg results
remain distinct from a normal Ubuntu desktop because compositor, shell,
device, and monitor behavior can differ.

Linux hotkey/audio/tray probes must be explicit. Pilot synthetic keypresses are
not evidence for native global-hotkey behavior.

### Hosted macOS

The `macos-15` runner performs frontend/Rust builds, AppKit smoke tests, debug
Tauri runtime smoke, package structure checks, `Info.plist` validation, and
ad-hoc signature verification. A small process/startup/IPC/clean-exit smoke is
preferred over fake GUI assertions.

Hosted CI is classified as partial for menu bar/tray, global hotkeys after
TCC/Accessibility approval, CoreAudio hardware behavior, Retina/multi-monitor
geometry, and Gatekeeper. Those limitations are recorded in the release
evidence and not hidden behind a green GUI test.

The workflow must record `uname -m` and `file` output for the produced binary.
Universal builds remain out of scope until the supported distribution
architectures are explicitly selected.

## E2E evidence contract

The existing WDIO suite is retained and strengthened with:

- JUnit reporter for every run.
- Required manifest listing each expected spec, result, timing, screenshot or
  snapshot path on failure, and log paths.
- Zero uncaught frontend error assertion in the runtime/surface contract.
- Backend error capture with an explicit allowlist for expected degraded audio
  states.
- Fail-closed artifact checks in both shell wrappers and CI uploads.
- A provider matrix that explicitly selects embedded or `tauri-driver`; an
  unavailable provider is a diagnostic failure, not a silent fallback.
- Numeric p95 budgets only for deterministic bootstrap/IPC operations after a
  baseline is recorded. Non-deterministic OS/audio metrics remain diagnostic.

Tauri Pilot remains local/debug-only. It uses an isolated socket/window,
explicit toolchain instructions, safe cleanup, screenshots, and logs. Pilot
availability never changes a production release exit code.

## CI and release flow

### Pull requests

- Shared checks, frontend tests/build, Rust checks, config contract, and
  Windows WDIO.
- Linux/macOS runtime jobs are not duplicated on every PR by default; changes
  to shared Rust/Tauri/frontend/E2E/scripts/workflows can opt into the full
  matrix through broad path matching.

### Master merges

- Full Windows/Linux/macOS runtime and package validation matrix.
- Upload structured artifacts and platform metadata.
- Preserve existing GTK/AppKit/core tests and release build checks.

### Release

- A tag must not directly create an unvalidated artifact.
- Prefer a reusable validation workflow whose successful artifacts are keyed to
  the exact commit SHA, then let the release job promote those artifacts.
- If artifact promotion is unavailable, the release workflow must run the full
  validation as a hard prerequisite and fail closed.
- Do not rerun Pilot in the release path.
- Public macOS distribution requires a future protected Developer ID and
  notarization path with temporary keychain import, `notarytool`, stapling,
  `spctl`, cleanup, and no secrets in repository files. The current ad-hoc path
  remains explicitly non-production distribution.

## Cost and flakiness controls

- Keep one WDIO worker and condition-based waits.
- Reuse npm/Cargo caches and avoid rebuilding the frontend solely for a second
  equivalent check.
- Avoid running the same E2E suite again after artifact promotion.
- Keep macOS full validation on merges/releases unless a broad shared-code
  change requires earlier execution.
- Keep Pilot local-only.
- Treat missing evidence and orphaned processes as failures, not warnings.

## Acceptance criteria for the implementation plan

The subsequent implementation plan must:

1. List only files and tests that still need changes after baseline
   reconciliation.
2. Include the orchestration reproducibility fixes and a clean-checkout test.
3. Separate debug WDIO validation from production binary/package validation.
4. Define the JUnit, manifest, error, timing, and cleanup contracts.
5. Define the Windows PR, full merge matrix, WSLg manual, and hosted macOS
   evidence boundaries.
6. Close the release bypass by requiring successful SHA-matched validation.
7. Keep Pilot out of the release dependency graph.
8. Include rollback points and the repository quality-gate commands.

## Approval gate

This document is a design/specification only. No source, workflow, profile, or
release behavior is changed by this document. Implementation planning begins
only after this spec is reviewed and approved.
