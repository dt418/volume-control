---
name: cross-platform-unix-guard
description: Prevent Unix (Linux/macOS) CI build failures and E2E test deadlocks in volume-control. Use when touching src-tauri lifecycle, Unix signal handling, platform cfgs, or Settings surface E2E tests.
---

# Cross-Platform Unix Compatibility Guard

VolumeControl builds and tests across Windows (MSVC), Ubuntu 24.04 (GTK4/libadwaita),
and macOS (AppKit/WebKit). Differences in compiler warning policies, platform-conditional
code, and WebDriver surface state can break Linux/macOS jobs silently on Windows hosts.

This skill encodes invariants preventing the three classes of Unix breakages seen in PR #72:
1. `-D unused-variables` on platform-conditional arguments.
2. `clippy::never_loop` on Unix termination signal listeners.
3. Multi-surface E2E state leakage (unisolated settings tabs).

## Invariants (do not violate)

### 1. Cross-target parameter suppression
Under `-D warnings`, any parameter or variable used exclusively under `#[cfg(windows)]`
(or any other specific OS target) will cause compilation to fail with `unused-variables` on Unix.
- Prefix the parameter name with an underscore (e.g. `_version: &str`), OR
- Suppress conditionally at the top of the function: `let _ = version;`.
- Never leave target-gated variables unconsumed in common functions.

### 2. Signal handler single-iteration loops
`signal_hook::iterator::Signals::forever()` is an infinite blocking iterator.
Using a `for sig in signals.forever() { ... break; }` loop triggers Clippy's default-deny
`clippy::never_loop` error on Unix.
- Use `if let Some(sig) = signals.forever().next() { ... }` instead of a `for` loop with `break`.

### 3. Settings E2E section isolation
In Tauri WebDriver tests (`e2e/tauri/specs/settings.e2e.ts`), the Settings surface remains
alive across tests within the suite. If a prior test switches to "Hotkeys" or another section,
subsequent tests must explicitly click the target section navigation button:
```typescript
await $(`//nav[@aria-label="Settings sections"]//button[normalize-space()="General"]`).click();
```
Never assume that the Settings surface opens on "General" by default in subsequent assertions.

### 4. Windows-host cross-target hygiene
When developing on Windows:
- Native `cargo check --target x86_64-unknown-linux-gnu` requires GNU C and GTK sysroots.
- Use `powershell -File .agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1` for local checks.
- Always run the Unix compat guard before committing:
  - Windows: `powershell -File scripts/check-unix-compat.ps1`
  - POSIX/CI: `bash scripts/check-unix-compat.sh`

## Workflow

1. Before committing changes touching `src-tauri` or `e2e/tauri`:
   - Run `powershell -File scripts/check-unix-compat.ps1` (or `bash scripts/check-unix-compat.sh`).
2. Run the full format-lint gate:
   - `powershell -File .agents/skills/format-lint/scripts/format-lint.ps1`.
3. Check CI platform matrix status early on cross-platform PRs.

## References

- `scripts/check-unix-compat.sh` / `.ps1` — automated invariant checks.
- `src-tauri/src/lifecycle.rs` — signal hook & cross-platform helper patterns.
- `e2e/tauri/specs/settings.e2e.ts` — section isolation pattern.
