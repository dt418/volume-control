# Tauri WebDriver and Tauri Pilot Testing Integration

**Date:** 2026-08-14
**Status:** Approved design
**Scope:** Windows-first E2E quality, followed by Linux/macOS parity

## Context

VolumeControl already has Rust unit tests, React/Vitest tests, a Win32 surface
verifier, and release-process evidence. Those layers do not yet provide a
standard WebView interaction harness for the real Tauri application. Tauri's
current testing guidance recommends WebdriverIO with `@wdio/tauri-service` for
desktop E2E; the service supports Windows, Linux, and macOS and can provide
embedded WebDriver, Tauri API execution, IPC mocking, and log capture. The
official guidance also documents Tauri's mock runtime for Rust integration
tests.

Tauri Pilot is a separate debug-oriented tool. Its plugin opens a Unix socket
or Windows named pipe, its CLI sends JSON-RPC commands, and its injected bridge
supports accessibility snapshots, interaction, assertions, screenshots,
console/network logs, recording/replay, declarative TOML scenarios, and an MCP
server. It is not a production runtime dependency.

Upstream references:

- Tauri tests: https://v2.tauri.app/develop/tests/
- Tauri WebDriver overview: https://v2.tauri.app/develop/tests/webdriver/
- Tauri WebDriver CI: https://v2.tauri.app/develop/tests/webdriver/ci/
- Official plugins workspace: https://github.com/tauri-apps/plugins-workspace
- Tauri Pilot: https://github.com/mpiton/tauri-pilot

## Goals

1. Add real Tauri WebView E2E coverage for Mixer, Settings, Help, recovery,
   multi-window lifecycle, and IPC interactions.
2. Make accessibility snapshots, screenshots, frontend/backend logs, and JUnit
   results first-class CI artifacts.
3. Provide Tauri Pilot for fast local inspection, scenario recording/replay, and
   AI-assisted triage without increasing the production attack surface.
4. Keep the existing Rust/Vitest/Win32 release checks and make WebDriver E2E a
   Windows release gate before Linux/macOS expansion.
5. Preserve the existing Rust 1.82 production toolchain unless a separate debug
   toolchain is explicitly selected for Pilot.

## Non-goals

- Do not replace the existing Win32 verifier or claim that WebDriver proves
  global OS hotkey latency.
- Do not ship the Pilot plugin, Pilot capability, debug socket, or test-only
  commands in a production build.
- Do not use `tauri-pilot press` as proof of X11 global-shortcut behavior; its
  upstream documentation identifies modifier-state limitations for X11 passive
  grabs. Use the existing host probe/IPC path for that coverage.
- Do not make a flaky interactive desktop test the only release evidence.

## Chosen architecture

### Production and debug separation

The production `default` capability remains unchanged. Test-only capabilities
are placed in a separate debug capability file and loaded only for a debug
test build. Cargo features are used to keep test plugins out of release
linking:

- `e2e-wdio`: enables the official Tauri WebDriver plugins required by the
  selected WebdriverIO provider.
- `e2e-pilot`: enables `tauri-plugin-pilot` in debug builds only.

The debug build is launched with an isolated `VOLUMECTL_CONFIG_DIR` and an
explicit test marker. The marker is checked before enabling either debug
plugin. A production binary with the marker still refuses test-plugin startup.

Tauri Pilot currently declares Rust 1.95+ and edition 2024 requirements. The
project's production toolchain remains Rust 1.82; Pilot-enabled debug builds
must therefore use an explicit Rust 1.95+ toolchain/profile or remain disabled
until the workspace is intentionally upgraded. A dependency resolution failure
must fail the debug E2E job clearly, never silently fall back to production.

### WebDriver provider strategy

The primary local and Windows CI provider is the official WebdriverIO Tauri
service. The embedded provider is preferred where supported because it avoids
an external driver process and is the documented path for macOS. Windows/Linux
may use the service's `tauri-driver` route when a native WebDriver is required;
the service manages the matching Edge driver on Windows. The repository's
existing release verifier remains responsible for decorated/non-client
geometry and PrintWindow evidence that DOM WebDriver cannot establish.

### Repository layout

```text
e2e/
  tauri/
    wdio.conf.ts
    package.json
    specs/
      mixer.e2e.ts
      settings.e2e.ts
      help.e2e.ts
      recovery.e2e.ts
      windows.e2e.ts
    support/
      app-fixture.ts
      artifacts.ts
      selectors.ts
  pilot/
    scenarios/
      mixer.toml
      settings.toml
      help.toml
      recovery.toml
    README.md
scripts/
  verify-tauri-e2e.ps1
  verify-tauri-e2e.sh
src-tauri/
  capabilities/default.json
  capabilities/e2e-wdio.json
  capabilities/e2e-pilot.json
```

The E2E package owns its WebdriverIO dependencies. The frontend runtime
package remains free of desktop-driver libraries.

## E2E scenario contract

All scenarios run one worker, use condition-based waits, reset their temporary
config, and save artifacts on failure.

### Mixer

- Open the diagnostic Mixer surface and assert title, bounded shell, header,
  System Output, session region, footer, and accessible control names.
- Fill search, verify filtering, change system volume through the slider, invoke
  mute/reset, and assert the resulting UI state.
- Exercise empty sessions, unsupported sessions, bootstrap error, and Escape
  close. Assert that the surface remains readable and the process remains alive.

### Settings

- Assert all six sections and the title/subtitle/close action.
- Edit General, Hotkeys, Appearance, Blacklist, Feedback, and Storage fields;
  verify draft-only behavior, validation feedback, Save, Reset, and Cancel.
- Assert focus order, disabled Save when clean, `aria-live` status, and retryable
  bootstrap errors.

### Help

- Assert shortcut cards, status badges, conflict callout, search, Settings CTA,
  footer actions, Escape, and bounded scroll/footer separation.
- Capture a light and dark appearance screenshot and check that the accessibility
  tree still exposes all primary controls.

### Recovery and multi-window

- Use WebDriver IPC mocking to reject `get_bootstrap`; assert a non-empty alert,
  readiness behavior, and no unhandled frontend error.
- Open Mixer, Settings, and Help through the diagnostic selector, enumerate
  windows, verify labels/titles, and close only the test-owned windows.
- Use `browser.tauri.execute()`/Pilot `eval` only for test inspection and timing;
  application state remains owned by Rust commands/events.

### Hotkeys and performance

WebDriver checks the deterministic IPC/action path. The real OS registration,
modifier handling, and p95 latency remain in the existing Win32 hotkey probe.
The E2E suite records:

- bootstrap-to-ready duration;
- command round-trip and state-event-to-render duration;
- screenshot and snapshot generation duration;
- frontend/backend error logs.

Performance reports include median and p95 but use hard gates only for the
already-approved deterministic budgets. E2E never converts a missing desktop
driver or synthetic X11 limitation into a pass.

## Tauri Pilot workflow

Pilot is installed as a developer tool (`cargo install tauri-pilot-cli` or a
documented prebuilt binary) and is not installed by the production package.
Debug startup requires the `pilot:default` capability and an explicit marker.

The standard local loop is:

```text
tauri-pilot ping
tauri-pilot windows
tauri-pilot snapshot -i --json
tauri-pilot click @ref
tauri-pilot fill @ref "value"
tauri-pilot assert visible @ref
tauri-pilot logs --level error --json
tauri-pilot screenshot --output output/tauri-pilot/surface.png
tauri-pilot record start
tauri-pilot record stop --output output/tauri-pilot/session.json
tauri-pilot replay output/tauri-pilot/session.json
tauri-pilot run e2e/pilot/scenarios/settings.toml --junit output/tauri-pilot/settings.xml
```

Pilot snapshots and records are exploratory/debug evidence. Stable assertions
that must gate release are ported to WebdriverIO specs and the existing release
verifier. Pilot's MCP server is opt-in and uses an explicit socket/window
selector so it cannot attach to an unrelated VolumeControl instance.

## CI and release policy

### Windows first

The Windows job builds a debug E2E binary, starts the WebDriver provider, runs
all WDIO specs, runs Pilot smoke only when the debug toolchain is available,
then builds the production binary and runs the existing surface/hotkey/release
verifiers. Artifacts include JUnit, snapshots, screenshots, logs, process
records, and latency JSON.

### Linux follow-up

The Linux job installs WebKitGTK/WebDriver dependencies and runs WDIO under
`xvfb-run`. Pilot runs only as an optional debug job; X11 global-shortcut tests
continue to use IPC/host probes. Parser and host-independent Rust tests remain
mandatory on Linux.

### macOS follow-up

The macOS job uses the embedded WebDriver provider because the official docs do
not provide the same native desktop WebDriver route as Windows/Linux. Pilot is
manual/debug evidence unless the socket and accessibility bridge are stable on
the runner. AppKit host smoke and parser tests remain mandatory.

### Fail-closed release

Production release does not depend on Pilot availability. It fails when WDIO
gate tests, existing Rust/frontend gates, surface verifier, hotkey probe,
artifact generation, or process-crash checks fail. Debug-only plugin build
failures are reported in the debug E2E job and never hidden.

## Security and cleanup

- Test capabilities are separate from `default.json`; no Pilot permission is
  present in production.
- Pilot sockets/named pipes use a per-run identifier and are not exposed to
  external network interfaces.
- E2E processes receive isolated config directories and are terminated by PID
  in `finally`; unrelated app instances are never killed.
- Screenshots/logs may contain user data and stay under untracked `output/`.
- CI uploads artifacts only on failure or explicit diagnostic runs.

## Acceptance criteria

- Windows WDIO E2E passes for Mixer, Settings, Help, recovery, and window
  lifecycle with zero unhandled frontend/backend errors.
- Pilot can connect to an explicitly marked debug binary, produce an
  accessibility snapshot, execute a Settings interaction, and replay a saved
  scenario; failure to connect is visible and never affects production build.
- Production binary has no Pilot dependency/capability and still passes the
  existing release verifier and full quality gate.
- Linux/macOS host-independent tests and platform-appropriate WDIO jobs are
  green before claiming cross-platform parity.
