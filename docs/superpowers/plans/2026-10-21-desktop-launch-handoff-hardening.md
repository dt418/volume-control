# Desktop Launch Handoff Hardening Implementation Plan (0.1.9)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship VolumeControl 0.1.9 with a desktop launch that always opens the mixer, a launch path that never blocks the UI on the `AppCore` lock, an installer that replaces running instances, and winget manifests that point at 0.1.9.

**Architecture:** Keep the existing Windows named-event handoff (already runtime-verified) and fix its semantics: launch mode is parsed once, background launches no longer request the mixer, a migrated login entry suppresses the current popup, and the mixer open is dispatched outside the 20 ms poll's core lock. Add a temporary regression script that exercises the real binary, an NSIS pre-install hook that terminates a running instance before file replacement, and a version-sync gate before the release workflow can build. Cross-platform single-instance ownership and the residual hotkey open-under-lock path are deferred to a separate 0.1.10 plan because they change the singleton architecture rather than repair it.

**Tech Stack:** Rust 2021 (Tauri 2, windows-sys 0.52), TypeScript/React 19 + Vitest, NSIS via Tauri bundler, PowerShell 5.1/7, GitHub Actions, winget 1.12 manifests.

## SDLC Mapping

| SDLC phase | This plan | Exit gate |
| --- | --- | --- |
| Requirements / analysis | Findings from Session 123 and PR #67 review | Each finding has a task or an explicit residual-risk entry |
| Design | `Architecture` + per-task `Interfaces` | Signatures are fixed before coding |
| Implementation | Tasks 0-7, TDD per task | Red test observed before each fix |
| Verification | Task 7 plus the full repository gate | New binary regression test green on Windows; all existing suites green |
| Release | Task 8 | Tag `v0.1.9`, immutable assets, winget validation |
| Maintenance | Out-of-scope section | Follow-up plan filed; no silent scope loss |

**Routing (ask-matt):** this is the bug-fix on-ramp after `/diagnosing-bugs`; the findings are already evidence-backed, so the flow continues at `/to-spec` → `/to-tickets` → `/implement`. This document is the ticket source; execute it with `superpowers:subagent-driven-development` (one task per subagent) or `superpowers:executing-plans` inline.

## Records Protocol (required by AGENTS.md)

`scripts/check-records.sh --staged` fails any commit whose change set contains a
substantive path unless the same staged set includes both `feature_list.json`
and `claude-progress.md`. Task 0 creates record `vol-104` and Session 124.
Before **every** task commit below:

1. Append one concrete evidence line to `vol-104.evidence` (the test command
   and observed result).
2. Append one matching bullet under Session 124 in `claude-progress.md`.
3. Stage both records in the same commit.

Never use `git commit --no-verify` to bypass this.

## Global Constraints

- Foreground launch = no arguments or only `--background` tokens; `--background` = tray-only, never opens a surface.
- Windows is the only platform with the event handoff in this release; Linux/macOS behavior must not regress.
- No new runtime dependencies in this release. No new `unsafe` outside `src-tauri/src/native_win32.rs`.
- All version fields are exactly `0.1.9`: workspace `Cargo.toml`, `crates/volumectl/Cargo.toml`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `frontend/package.json`, `frontend/package-lock.json`, `Cargo.lock`.
- Never rewrite an autostart Run value that points at a different executable.
- Tests must fail before the implementation and pass after it. Do not add sleeps longer than the bounded polling helpers in `scripts/verify-tauri-surfaces.ps1`.
- Run the full gate from `AGENTS.md` before each commit that touches Rust or Tauri surface code:
  `cargo fmt --all --check`, `git diff --check`,
  `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`,
  `cargo test --workspace --no-default-features`,
  `sh scripts/check-tauri-deadlock.sh` (surface changes),
  `sh scripts/check-records.sh --staged` (staged records).
- Tagging, merging to `main`, and updating the external winget PR are publish actions. Execute them only after the user explicitly approves the release step.

---

### Task 0: Register the hardening work in the repository records

**Files:**
- Modify: `feature_list.json`
- Modify: `claude-progress.md`

**Interfaces:**
- Produces: record `vol-104` and Session 124, both consumed by every later task's commit.

- [ ] **Step 1: Add `vol-104`**

Insert this object at the top of the `features` array and set `last_updated`
to the current local timestamp:

```json
    {
      "id": "vol-104",
      "priority": 104,
      "area": "windows-host",
      "title": "Harden the desktop launch handoff and release 0.1.9",
      "user_visible_behavior": "Foreground launches always open the mixer, background launches stay tray-only, installer updates replace a running instance, and the 0.1.9 release cannot be tagged with mismatched version fields.",
      "status": "in_progress",
      "verification": [],
      "evidence": [
        "Session 124: fixes target the launch-mode, migration, core-lock, stale-close, NSIS, and version-drift findings from Session 123."
      ],
      "notes": "Completes PR #67 before the 0.1.9 tag; cross-platform single-instance is deferred to 0.1.10."
    },
```

- [ ] **Step 2: Add Session 124 to `claude-progress.md`**

Insert under `# Progress Log`:

```markdown
## Session 124 (2026-10-21) - Harden the desktop launch handoff for 0.1.9

- Goal: eliminate the duplicate-background popup, first-login popup, core-lock
  stall, stale mixer close, version-skew update, and tag/version drift found in
  Session 123.
- Plan: `docs/superpowers/plans/2026-10-21-desktop-launch-handoff-hardening.md`.
- Evidence: filled task-by-task as each commit lands.
```

- [ ] **Step 3: Verify and commit**

```bash
python -c "import json; json.load(open('feature_list.json'))"
git add feature_list.json claude-progress.md
sh scripts/check-records.sh --staged
git commit -m "docs(records): register the 0.1.9 launch hardening"
```

Expected: JSON parses; the guard passes because only records are staged.

---

### Task 1: Centralize host and background launch mode

**Files:**
- Modify: `crates/volumectl/src/autostart.rs` (after `BACKGROUND_FLAG`, around line 24)
- Modify: `src-tauri/src/main.rs:8-16`
- Modify: `src-tauri/src/lib.rs:306-314`
- Test: `crates/volumectl/src/autostart.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `pub fn is_background_launch(args: &[String]) -> bool` — true when any argument equals `--background`.
- Produces: `pub fn is_host_launch(args: &[String]) -> bool` — true when args are empty or every arg equals `--background`.
- Consumes: `BACKGROUND_FLAG: &str = "--background"`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/volumectl/src/autostart.rs` tests:

```rust
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn launch_mode_accepts_only_known_host_arguments() {
        assert!(super::is_host_launch(&args(&[])));
        assert!(super::is_host_launch(&args(&["--background"])));
        assert!(super::is_host_launch(&args(&[
            "--background",
            "--background"
        ])));
        assert!(!super::is_host_launch(&args(&["get"])));
        assert!(!super::is_host_launch(&args(&["--background", "get"])));
    }

    #[test]
    fn background_launch_detects_the_flag_anywhere() {
        assert!(!super::is_background_launch(&args(&[])));
        assert!(super::is_background_launch(&args(&["--background"])));
        assert!(super::is_background_launch(&args(&[
            "--profile",
            "--background"
        ])));
        assert!(!super::is_background_launch(&args(&["--background-lite"])));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p volumectl --lib autostart::tests::launch_mode -- --nocapture`
Expected: FAIL — `is_host_launch` / `is_background_launch` not found.

- [ ] **Step 3: Implement the helpers**

Add below `pub const BACKGROUND_FLAG`:

```rust
/// True when any host argument requests the tray-only launch mode.
pub fn is_background_launch(args: &[String]) -> bool {
    args.iter().any(|arg| arg.as_str() == BACKGROUND_FLAG)
}

/// True when the argument list is a host launch rather than a CLI verb.
/// An empty list is a foreground launch; only background-flag tokens are
/// treated as host arguments, so `--background foo` stays a CLI invocation.
pub fn is_host_launch(args: &[String]) -> bool {
    args.is_empty() || args.iter().all(|arg| arg.as_str() == BACKGROUND_FLAG)
}
```

Replace the body of `main` in `src-tauri/src/main.rs` up to the `if !host_launch` with:

```rust
fn main() -> std::process::ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    // `--background` is a host launch mode (the login autostart registration
    // uses it), not a CLI verb: the host must start tray-only instead of the
    // one-shot CLI exiting before any window can open.
    if !volumectl_lib::autostart::is_host_launch(&args) {
        return match volumectl_lib::cli::run() {
            Ok(code) => code,
            Err(e) => {
                eprintln!("volumectl: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    volumecontrol_tauri_lib::run().expect("VolumeControl Tauri host failed to start");
    std::process::ExitCode::SUCCESS
}
```

In `src-tauri/src/lib.rs`, move the argument parsing to the top of the setup closure, before the single-instance block (around line 240), and delete only the later `let background_launch = std::env::args()...` statement at lines 311-313. Keep the following `if !background_launch && ...` startup-open block; it now uses the earlier binding.

```rust
            let args = std::env::args().skip(1).collect::<Vec<_>>();
            let mut background_launch =
                volumectl_lib::autostart::is_background_launch(&args);
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p volumectl --lib autostart`
Expected: PASS.

Run: `cargo check -p volumecontrol-tauri`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/volumectl/src/autostart.rs src-tauri/src/main.rs src-tauri/src/lib.rs \
  feature_list.json claude-progress.md
git commit -m "refactor(launch): centralize host and background launch mode"
```

---

### Task 2: Respect background launches and migrated login entries

**Files:**
- Modify: `crates/volumectl/src/autostart.rs` (`ensure_background_flag`, `should_migrate`)
- Modify: `src-tauri/src/native_win32.rs:74-92`
- Modify: `src-tauri/src/lib.rs:240-266`
- Test: `crates/volumectl/src/autostart.rs` (`windows_registry::tests`)

**Interfaces:**
- Changes: `pub fn ensure_background_flag() -> Result<(), String>` becomes `Result<bool, String>`; `true` means the Run value was rewritten for this launch.
- Changes: `pub fn ensure_single_instance() -> bool` becomes `pub fn ensure_single_instance(show_mixer: bool) -> bool`.
- Consumes: `background_launch` from Task 1; `SHOW_MIXER_EVENT_NAME` unchanged.

- [ ] **Step 1: Write the failing Windows-only test**

Inside `windows_registry::tests` add:

```rust
        #[test]
        fn migration_targets_only_unflagged_matching_commands() {
            let executable = Path::new(r"C:\VolumeControl.exe");
            assert!(should_migrate(
                r#""C:\VolumeControl.exe""#,
                executable
            ));
            assert!(!should_migrate(
                r#""C:\VolumeControl.exe" --background"#,
                executable
            ));
            assert!(!should_migrate(
                r#""C:\Other.exe""#,
                executable
            ));
        }
```

Add `should_migrate` to the test import:

```rust
        use super::{command_matches_executable, command_token, should_migrate};
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p volumectl --lib windows_registry::tests::migration -- --nocapture`
Expected: FAIL — `should_migrate` not found (on Windows; on Linux this module is not compiled).

- [ ] **Step 3: Implement migration detection and return the rewritten flag**

In `windows_registry`, replace `ensure_background_flag` with:

```rust
    fn should_migrate(command: &str, executable: &Path) -> bool {
        !has_background_flag(command) && command_matches_executable(command, executable)
    }

    /// Rewrite an existing login entry that predates the background flag so
    /// an upgraded user does not get the mixer popping up at every sign-in.
    /// Returns `true` only when this launch rewrote the value, which the host
    /// uses to keep the current login launch tray-only.
    pub(super) fn ensure_background_flag() -> Result<bool, String> {
        let Some(key) = open_run_key(KEY_READ)? else {
            return Ok(false);
        };
        let Some(command) = read_run_value(&key)? else {
            return Ok(false);
        };
        drop(key);
        let executable = std::env::current_exe()
            .map_err(|error| format!("could not determine current executable: {error}"))?;
        if !should_migrate(&command, &executable) {
            return Ok(false);
        }
        set_enabled(true)?;
        Ok(true)
    }
```

Change the public wrappers:

```rust
#[cfg(windows)]
pub fn ensure_background_flag() -> Result<bool, String> {
    windows_registry::ensure_background_flag()
}

#[cfg(not(windows))]
pub fn ensure_background_flag() -> Result<bool, String> {
    Ok(false)
}
```

In `native_win32.rs`, change the signature and guard the signal:

```rust
pub fn ensure_single_instance(show_mixer: bool) -> bool {
    unsafe {
        let h = CreateMutexW(
            std::ptr::null(),
            1,
            windows_sys::core::w!("Local\\VolumeControl.SingleInstance"),
        );
        if h == 0 {
            return true;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(h);
            log::warn!("another VolumeControl instance is already running");
            if show_mixer {
                request_show_mixer();
            }
            return false;
        }
        create_show_mixer_event();
        true
    }
}
```

In `lib.rs`, call it with the parsed foreground flag:

```rust
            if !debug_e2e_instance
                && !native_win32::ensure_single_instance(!background_launch)
            {
```

Replace the migration call site at lines 260-264 with:

```rust
            match volumectl_lib::autostart::ensure_background_flag() {
                Ok(migrated) => background_launch |= migrated,
                Err(error) => log::warn!("could not refresh the auto-start command: {error}"),
            }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p volumectl --lib autostart`
Expected: PASS on Windows; `windows_registry` tests are skipped elsewhere.

Run: `cargo check -p volumecontrol-tauri`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/volumectl/src/autostart.rs src-tauri/src/native_win32.rs src-tauri/src/lib.rs \
  feature_list.json claude-progress.md
git commit -m "fix(host): keep background launches tray-only"
```

---

### Task 3: Dispatch the show-mixer request outside the core lock

**Files:**
- Modify: `src-tauri/src/lib.rs:345-365`
- Test: `src-tauri/src/native_win32.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `native_win32::take_show_mixer_request() -> bool`, `WindowManager::open(SurfaceId::Mixer)`.
- Produces: no change to `AppCore` or `TauriSink`.

- [ ] **Step 1: Write the failing event test**

Append to `src-tauri/src/native_win32.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{create_show_mixer_event, request_show_mixer, take_show_mixer_request};

    #[test]
    fn show_mixer_event_is_auto_reset() {
        create_show_mixer_event();
        assert!(!take_show_mixer_request(), "fresh event must be clear");
        request_show_mixer();
        assert!(take_show_mixer_request(), "signalled event must be observed");
        assert!(
            !take_show_mixer_request(),
            "auto-reset event must clear after one wait"
        );
    }
}
```

- [ ] **Step 2: Confirm the transport test is green and record the lock-order red condition**

Run: `cargo test -p volumecontrol-tauri show_mixer_event -- --nocapture`
Expected: PASS. This locks the event semantics but does not expose the lock
order. The lock-order defect is a contention race: reproduce it only through
the runtime gate in Task 7 while a sync command is in flight. If you cannot
reproduce the stall locally, treat the code move as a reviewed invariant
(`take_show_mixer_request` and `WindowManager::open` must appear before
`try_lock`) and keep the exact ordering in the diff.

- [ ] **Step 3: Move the handler before `try_lock`**

Replace the fast-poll closure in `src-tauri/src/lib.rs` with:

```rust
            let fast_shared = shared.clone();
            let fast_handle = handle.clone();
            #[cfg(target_os = "windows")]
            let fast_native = native.clone();
            std::thread::spawn(move || loop {
                // A second launch must never wait for the core mutex: a sync
                // Tauri command on the main thread can hold it while the
                // window manager waits for the main thread. Dispatch the
                // surface request first, with no core lock held.
                #[cfg(target_os = "windows")]
                if native_win32::take_show_mixer_request() {
                    match fast_handle.try_state::<WindowManager>() {
                        Some(manager) => {
                            if let Err(error) = manager.open(SurfaceId::Mixer) {
                                log::warn!("show-mixer request failed: {error}");
                            }
                        }
                        None => log::warn!(
                            "show-mixer request arrived before the window manager was managed"
                        ),
                    }
                }
                // Never queue an IPC command behind a poll: when a command
                // holds the core (volume slider, bootstrap, tray action),
                // skip this cycle instead of blocking.
                let Ok(mut core) = fast_shared.try_lock() else {
                    std::thread::sleep(Duration::from_millis(FAST_POLL_MS));
                    continue;
                };
                core.poll_hotkeys();
                #[cfg(target_os = "windows")]
                while let Some(action) = fast_native.try_recv_wheel() {
                    core.apply_hotkey(action);
                }
                drop(core);
                std::thread::sleep(Duration::from_millis(FAST_POLL_MS));
            });
```

Delete the old `take_show_mixer_request` block that sat inside the locked section.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p volumecontrol-tauri show_mixer_event -- --nocapture`
Expected: PASS.

Run: `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/native_win32.rs \
  feature_list.json claude-progress.md
git commit -m "fix(host): open the mixer without holding the core lock"
```

---

### Task 4: Ignore stale mixer close requests

**Files:**
- Modify: `frontend/src/mixer/MixerSurface.tsx:51-59`
- Test: `frontend/src/mixer/MixerSurface.test.tsx:96-104`

**Interfaces:**
- Consumes: Tauri event `close_mixer_request`; `document.hasFocus()`.
- Produces: unchanged IPC command `close_surface`.

- [ ] **Step 1: Write the failing test**

Replace the existing blur test with two tests:

```tsx
  it("closes the window when the Rust side emits close_mixer_request while unfocused", async () => {
    await renderSurface();
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    expect(listeners["close_mixer_request"]).toBeDefined();
    listeners["close_mixer_request"](null);
    expect(ipc.invoke).toHaveBeenCalledWith("close_surface", {
      surface: "window-mixer",
    });
  });

  it("ignores a stale close request after the mixer regains focus", async () => {
    await renderSurface();
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    listeners["close_mixer_request"](null);
    expect(ipc.invoke).not.toHaveBeenCalledWith("close_surface", {
      surface: "window-mixer",
    });
  });
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix frontend test -- MixerSurface`
Expected: FAIL — the stale-close test sees `close_surface` called.

- [ ] **Step 3: Guard the handler**

Change the listener in `MixerSurface.tsx`:

```tsx
    void listen<void>("close_mixer_request", () => {
      // A close event can be queued while the window is losing focus and be
      // delivered after a second launch has reopened it. Only close if the
      // window is still unfocused.
      if (document.hasFocus()) return;
      void invoke<void>("close_surface", { surface: "window-mixer" }).catch(() => {});
    }).then((u) => {
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix frontend test -- MixerSurface`
Expected: PASS.

Run: `npm --prefix frontend run build`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/mixer/MixerSurface.tsx frontend/src/mixer/MixerSurface.test.tsx \
  feature_list.json claude-progress.md
git commit -m "fix(mixer): ignore stale blur close requests"
```

---

### Task 5: Stop a running instance before updating

**Files:**
- Create: `src-tauri/nsis/hooks.nsh`
- Modify: `src-tauri/tauri.conf.json` (`bundle`)
- Test: `scripts/test-release-workflow.sh`

**Interfaces:**
- Consumes: NSIS hook macro `NSIS_HOOK_PREINSTALL`, Tauri bundler.
- Produces: installer that terminates `VolumeControl.exe` before copying files.

- [ ] **Step 1: Write the failing contract test**

Add after the existing NSIS packaging contract in `scripts/test-release-workflow.sh`:

```bash
hook="$repo/src-tauri/nsis/hooks.nsh"
if [[ -f "$hook" ]] && \
   contains "$repo/src-tauri/tauri.conf.json" \
     '"installerHooks"[[:space:]]*:[[:space:]]*"nsis/hooks\.nsh"' && \
   contains "$hook" '!macro[[:space:]]+NSIS_HOOK_PREINSTALL' && \
   contains "$hook" 'taskkill\.exe" /IM VolumeControl\.exe'; then
  report ok 'NSIS preinstall hook stops the running VolumeControl instance'
else
  report FAIL 'NSIS preinstall hook stops the running VolumeControl instance'
fi
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `bash scripts/test-release-workflow.sh`
Expected: FAIL — hook file and config missing.

- [ ] **Step 3: Add the hook and configuration**

Create `src-tauri/nsis/hooks.nsh`:

```nsh
; Stop a running instance before replacing its executable. Without this, an
; update leaves the old process holding the file and the new binary cannot
; replace it; clicking the shortcut then has no listener for the new handoff.
!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Closing any running VolumeControl instance..."
  nsExec::Exec '"$SYSDIR\taskkill.exe" /IM VolumeControl.exe /F'
  Pop $0
!macroend
```

Add inside `bundle` in `src-tauri/tauri.conf.json`:

```json
    "windows": {
      "nsis": {
        "installerHooks": "nsis/hooks.nsh"
      }
    },
```

- [ ] **Step 4: Rebuild the installer and verify the hook ran**

Run the full gate, then:

```bash
frontend/node_modules/.bin/tauri build --bundles nsis --ci
```

Start the built host, then run the generated installer silently. Task 5 runs
before the version bump, so resolve the current name instead of hardcoding it:

```powershell
$installer = Get-ChildItem target/release/bundle/nsis/VolumeControl_*_x64-setup.exe |
  Sort-Object LastWriteTime -Descending | Select-Object -First 1
Start-Process target/release/VolumeControl.exe
Start-Sleep -Seconds 4
Start-Process $installer.FullName -ArgumentList "/S" -Wait
Get-Process VolumeControl -ErrorAction SilentlyContinue
```

Expected: `Get-Process` prints no process; the installer exits 0.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/nsis/hooks.nsh src-tauri/tauri.conf.json scripts/test-release-workflow.sh \
  feature_list.json claude-progress.md
git commit -m "fix(installer): stop a running instance before replacement"
```

---

### Task 6: Bump to 0.1.9 and gate tag/version drift

**Files:**
- Create: `scripts/check-version-sync.sh`
- Modify: `Cargo.toml:8`, `crates/volumectl/Cargo.toml:3`, `src-tauri/Cargo.toml:3`, `src-tauri/tauri.conf.json:5`
- Modify: `frontend/package.json`, `frontend/package-lock.json`
- Modify: `Cargo.lock` (regenerated)
- Modify: `.github/workflows/release.yml` (preflight)
- Modify: `scripts/test-release-workflow.sh`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Produces: `scripts/check-version-sync.sh <vX.Y.Z>` — exits 0 only when the tag matches every app version field; prints each mismatch.
- Consumes: `release_tag` output from the preflight resolver.

- [ ] **Step 1: Write the failing version-sync test**

Append to `scripts/test-release-workflow.sh`:

```bash
current_tag="v$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$repo/src-tauri/tauri.conf.json" | head -n 1)"
if bash "$repo/scripts/check-version-sync.sh" "$current_tag" >/dev/null 2>&1; then
  report ok 'version sync accepts the checked-in app version'
else
  report FAIL 'version sync accepts the checked-in app version'
fi
if bash "$repo/scripts/check-version-sync.sh" v0.0.0 >/dev/null 2>&1; then
  report FAIL 'version sync rejects a mismatched tag'
else
  report ok 'version sync rejects a mismatched tag'
fi
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `bash scripts/test-release-workflow.sh`
Expected: FAIL — `scripts/check-version-sync.sh` does not exist.

- [ ] **Step 3: Add the script and wire it into preflight**

Create `scripts/check-version-sync.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

tag="${1:?usage: $0 vX.Y.Z}"
case "$tag" in
  v[0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "version must look like vMAJOR.MINOR.PATCH: $tag" >&2; exit 2 ;;
esac
version="${tag#v}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
failures=0

check() {
  local label="$1" actual="$2"
  if [ "$actual" != "$version" ]; then
    echo "FAIL $label: expected $version, got ${actual:-<empty>}" >&2
    failures=$((failures + 1))
  fi
}

check "tauri.conf.json" "$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$root/src-tauri/tauri.conf.json" | head -n 1)"
check "workspace Cargo.toml" "$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' "$root/Cargo.toml" | head -n 1)"
check "crates/volumectl/Cargo.toml" "$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/crates/volumectl/Cargo.toml" | head -n 1)"
check "src-tauri/Cargo.toml" "$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/src-tauri/Cargo.toml" | head -n 1)"
check "frontend/package.json" "$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$root/frontend/package.json" | head -n 1)"
check "frontend/package-lock.json" "$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$root/frontend/package-lock.json" | head -n 1)"

if [ "$failures" -ne 0 ]; then
  exit 1
fi
echo "version sync ok: $version"
```

Add `actions/checkout` to the preflight job (it currently has none) and a step after the tag-format check and before the tag lookup. The checkout must be the first step of `preflight`:

```yaml
    steps:
      - uses: actions/checkout@v7
      - name: Resolve and validate release tag
        # existing step unchanged
```

Then add after the existing resolver step:

```yaml
      - name: Verify tag matches the app version
        shell: bash
        env:
          RELEASE_TAG: ${{ steps.release-tag.outputs.release_tag }}
        run: bash scripts/check-version-sync.sh "$RELEASE_TAG"
```

- [ ] **Step 4: Bump every version field**

Use the edit tool for each manifest; do not use `sed` on individual files:

- `Cargo.toml` line 8: `version = "0.1.9"`
- `crates/volumectl/Cargo.toml` line 3: `version = "0.1.9"`
- `src-tauri/Cargo.toml` line 3: `version = "0.1.9"`
- `src-tauri/tauri.conf.json` line 5: `"version": "0.1.9",`
- `frontend/package.json` line 3: `"version": "0.1.9",`
- `frontend/package-lock.json` lines 3 and 9: `"version": "0.1.9",`

Run:

```bash
cargo check --workspace --no-default-features
```

Expected: PASS; `Cargo.lock` now records `volumectl 0.1.9` and `volumecontrol-tauri 0.1.9`.

Move the `[Unreleased]` entries in `CHANGELOG.md` under:

```markdown
## [0.1.9] - 2026-10-21
```

and add:

```markdown
### Fixed

- The desktop/Start menu shortcut opens the mixer, including when another
  VolumeControl process is already running.
- Background launches (`--background`, including migrated login entries) no
  longer open a surface.
- Installer updates stop the running instance before replacing its executable.
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `bash scripts/check-version-sync.sh v0.1.9`
Expected: `version sync ok: 0.1.9`.

Run: `bash scripts/test-release-workflow.sh`
Expected: PASS, including both version-sync checks.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/volumectl/Cargo.toml src-tauri/Cargo.toml \
  src-tauri/tauri.conf.json frontend/package.json frontend/package-lock.json \
  scripts/check-version-sync.sh scripts/test-release-workflow.sh \
  .github/workflows/release.yml CHANGELOG.md \
  feature_list.json claude-progress.md
git commit -m "release: prepare 0.1.9 and gate tag/version drift"
```

---

### Task 7: Add the real-binary launch regression gate

**Files:**
- Create: `scripts/verify-desktop-launch.ps1`
- Modify: `.github/workflows/desktop-validation.yml` (Windows steps)
- Test: the script itself runs in CI

**Interfaces:**
- Consumes: `scripts/win32_pinvoke.cs` (`GetProcessWindows`, `IsWindowVisible`, `GetWindowTitle`), `VOLUMECTL_CONFIG_DIR`.
- Produces: exit 0 only when foreground launches show `Volume Mixer`, background launches stay windowless, and a duplicate background launch stays windowless.

- [ ] **Step 1: Write the regression script**

Create `scripts/verify-desktop-launch.ps1`:

```powershell
#!/usr/bin/env pwsh
param(
    [string]$Binary = 'target/debug/VolumeControl.exe',
    [string]$OutputRoot = 'output/desktop-launch',
    [int]$TimeoutSeconds = 20
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$binaryPath = [IO.Path]::GetFullPath((Join-Path $repoRoot $Binary))
if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
    throw "Binary not found: $binaryPath"
}
$outputPath = [IO.Path]::GetFullPath((Join-Path $repoRoot $OutputRoot))
New-Item -ItemType Directory -Force -Path $outputPath | Out-Null

$csFile = Join-Path $PSScriptRoot 'win32_pinvoke.cs'
$cscCandidates = @(
    "$env:SystemRoot\Microsoft.NET\Framework64\v4.0.30319\csc.exe",
    "$env:SystemRoot\Microsoft.NET\Framework\v4.0.30319\csc.exe"
)
$csc = $cscCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $csc) { throw 'csc.exe not found; Windows .NET Framework is required.' }
$dllPath = Join-Path ([IO.Path]::GetTempPath()) "volume-control-launch-$PID.dll"
& $csc /nologo /target:library /out:$dllPath /reference:System.Drawing.dll $csFile 2>&1 | Write-Host
if ($LASTEXITCODE -ne 0) { throw 'Failed to compile the Win32 helper.' }
Add-Type -Path $dllPath

$configDir = Join-Path ([IO.Path]::GetTempPath()) "volume-control-launch-$PID"
New-Item -ItemType Directory -Force -Path $configDir | Out-Null
$started = [System.Collections.Generic.List[Diagnostics.Process]]::new()

function Start-Host {
    param([string[]]$Arguments = @())
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $binaryPath
    $start.WorkingDirectory = $repoRoot
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.Arguments = ($Arguments -join ' ')
    $start.Environment['VOLUMECTL_CONFIG_DIR'] = $configDir
    $process = [Diagnostics.Process]::Start($start)
    $script:started.Add($process)
    return $process
}

function Get-MixerWindow([int]$ProcessId) {
    foreach ($handle in [Win32]::GetProcessWindows($ProcessId)) {
        if ([Win32]::IsWindowVisible($handle) -and
            [Win32]::GetWindowTitle($handle) -eq 'Volume Mixer') {
            return $handle
        }
    }
    return [IntPtr]::Zero
}

function Wait-MixerWindow([Diagnostics.Process]$Process, [int]$TimeoutMs) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    do {
        if ($Process.HasExited) { throw "Process exited early with code $($Process.ExitCode)." }
        $handle = Get-MixerWindow $Process.Id
        if ($handle -ne [IntPtr]::Zero) { return $handle }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "No visible Volume Mixer within $TimeoutMs ms."
}

function Wait-NoMixer([Diagnostics.Process]$Process, [int]$TimeoutMs) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    do {
        if ($Process.HasExited) { throw "Background process exited with code $($Process.ExitCode)." }
        if ((Get-MixerWindow $Process.Id) -ne [IntPtr]::Zero) {
            throw 'Background launch showed the Volume Mixer.'
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
}

try {
    Get-Process VolumeControl -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $binaryPath } |
        Stop-Process -Force

    $primary = Start-Host
    [void](Wait-MixerWindow $primary ($TimeoutSeconds * 1000))
    Write-Host 'PASS foreground launch opens the mixer'

    $second = Start-Host
    if (-not $second.WaitForExit(5000)) { throw 'Second foreground launch did not exit.' }
    if ($primary.HasExited) { throw 'Primary exited after the second launch.' }
    [void](Wait-MixerWindow $primary 5000)
    Write-Host 'PASS second foreground launch keeps the primary and the mixer'

    $primary | Stop-Process -Force
    $primary.WaitForExit(5000) | Out-Null

    $background = Start-Host @('--background')
    Wait-NoMixer $background 3000
    Write-Host 'PASS background launch stays windowless'

    $backgroundSecond = Start-Host @('--background')
    if (-not $backgroundSecond.WaitForExit(5000)) { throw 'Second background launch did not exit.' }
    Wait-NoMixer $background 3000
    Write-Host 'PASS duplicate background launch stays windowless'

    $foreground = Start-Host
    [void](Wait-MixerWindow $background ($TimeoutSeconds * 1000))
    Write-Host 'PASS foreground launch wakes a background primary'
} finally {
    foreach ($process in $started) {
        if (-not $process.HasExited) { $process.Kill() }
        $process.Dispose()
    }
}
```

- [ ] **Step 2: Run the script against the release candidate**

Run: `pwsh -NoProfile -File scripts/verify-desktop-launch.ps1 -Binary target/release/VolumeControl.exe`
Expected: five PASS lines, exit 0.

Negative check: run `git stash` on the Task 2 change and confirm the duplicate-background step fails, then restore. This proves the gate catches the bug.

- [ ] **Step 3: Add the CI step**

In `.github/workflows/desktop-validation.yml`, after the Windows E2E step and before the macOS step:

```yaml
      - name: Verify desktop launch handoff
        if: matrix.platform == 'windows'
        shell: pwsh
        run: ./scripts/verify-desktop-launch.ps1 -Binary target/debug/VolumeControl.exe -OutputRoot output/desktop-launch
```

- [ ] **Step 4: Run the local gates**

Run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
sh scripts/check-tauri-deadlock.sh
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add scripts/verify-desktop-launch.ps1 .github/workflows/desktop-validation.yml \
  feature_list.json claude-progress.md
git commit -m "test(host): gate desktop launch handoff on a real binary"
```

---

### Task 8: Release 0.1.9 and move winget to 0.1.9

**Files:**
- Modify: `winget/dt418.VolumeControl/0.1.9/*` (new), remove `0.1.8/*`
- Modify: `docs/index.html` only after the winget PR merges
- External: `microsoft/winget-pkgs` PR #454508

**Interfaces:**
- Consumes: release asset `VolumeControl_0.1.9_x64-setup.exe`, its SHA-256 from `SHA256SUMS.txt`.
- Produces: schema-valid 1.12.0 manifests and an updated PR body.

- [ ] **Step 1: Pre-release gate (human approval required)**

Confirm with the user, then run the full gate from `AGENTS.md`, plus:

```bash
bash scripts/test-release-workflow.sh
pwsh -NoProfile -File scripts/verify-desktop-launch.ps1 -Binary target/release/VolumeControl.exe
```

Do not tag until both pass.

- [ ] **Step 2: Merge and tag**

```bash
gh pr merge 67 --squash --delete-branch
git switch main
git pull --ff-only
git tag -a v0.1.9 -m "Release v0.1.9"
git push origin v0.1.9
gh run watch
```

Expected: release workflow succeeds; `gh release view v0.1.9` lists the NSIS installer and `SHA256SUMS.txt`.

- [ ] **Step 3: Generate and validate manifests**

```bash
mkdir -p output/winget-019
cd output/winget-019
curl -fL -o VolumeControl_0.1.9_x64-setup.exe \
  https://github.com/dt418/volume-control/releases/download/v0.1.9/VolumeControl_0.1.9_x64-setup.exe
curl -fsSL -o SHA256SUMS.txt \
  https://github.com/dt418/volume-control/releases/download/v0.1.9/SHA256SUMS.txt
sha256sum VolumeControl_0.1.9_x64-setup.exe
grep -i setup.exe SHA256SUMS.txt
bash ../../scripts/generate-winget-manifest.sh v0.1.9 \
  VolumeControl_0.1.9_x64-setup.exe manifests
winget.exe validate --manifest "$(cygpath -w manifests/dt418.VolumeControl/0.1.9)"
```

Expected: both hashes match; `Manifest validation succeeded.`

- [ ] **Step 4: Update the repository tree**

```bash
cd ../..
rm -rf winget/dt418.VolumeControl/0.1.8
cp -R output/winget-019/manifests/dt418.VolumeControl/0.1.9 winget/dt418.VolumeControl/
```

- [ ] **Step 5: Update the winget fork and PR**

```bash
git clone --depth 1 --filter=blob:none --sparse \
  --branch dt418.VolumeControl-0.1.5 \
  https://github.com/dt418/winget-pkgs.git output/winget-fork
cd output/winget-fork
git sparse-checkout set manifests/d/dt418/VolumeControl
git rm -r manifests/d/dt418/VolumeControl/0.1.8
mkdir -p manifests/d/dt418/VolumeControl/0.1.9
cp ../../winget/dt418.VolumeControl/0.1.9/*.yaml manifests/d/dt418/VolumeControl/0.1.9/
git add manifests/d/dt418/VolumeControl/0.1.9
git -c user.name=dt418 -c user.email=dt418@users.noreply.github.com \
  commit -m "Update dt418.VolumeControl to 0.1.9"
git push origin dt418.VolumeControl-0.1.5
gh pr edit 454508 -R microsoft/winget-pkgs \
  --title "New package: dt418.VolumeControl version 0.1.9"
```

Update the PR body's installer URL and SHA-256 to the 0.1.9 values. Wait for `winget validate` and the installation-validation checks; do not merge or claim publication before they pass.

- [ ] **Step 6: Commit the repository manifest change**

```bash
git add winget/dt418.VolumeControl feature_list.json claude-progress.md
git commit -m "chore(winget): submit 0.1.9 manifests"
```

---

## Verification Matrix

| Finding | Task | Test |
| --- | --- | --- |
| Desktop icon no-op on fresh launch | already fixed in PR #67; kept by Task 7 | `verify-desktop-launch.ps1` foreground step |
| Second launch silent exit | PR #67 + Task 2/3 | second-foreground step + event unit test |
| Duplicate `--background` opens mixer | Task 1/2 | duplicate-background step |
| First login after migration opens mixer | Task 2 | migration unit test + duplicate-background runtime step |
| Open under core lock stalls main thread | Task 3 | event moved outside `try_lock`; clippy + existing E2E |
| Stale blur closes reopened mixer | Task 4 | Vitest stale-close test |
| Version skew leaves old primary running | Task 5 | installer silent-update step |
| Tag/version drift fails release late | Task 6 | `check-version-sync.sh` tests + workflow step |
| Winget publishes a build without the fix | Task 8 | PR points at 0.1.9 installer; `winget validate` passes |

## Out of Scope / Follow-ups

- **Cross-platform single-instance ownership (0.1.10):** macOS/Linux still start a second host when the desktop entry is launched while the app runs. Use `tauri-plugin-single-instance` first-plugin registration, move the callback's mixer open to a ready-state retry, delete the Windows-only mutex/event once the plugin passes runtime coverage on all three platforms. Do not mix this into 0.1.9; it changes the singleton transport and needs its own plan.
- **Hotkey opens still run under the core lock:** `AppCore::poll_hotkeys` can call `WindowManager::open` while the 20 ms poll holds the mutex. The new event path no longer does. The hotkey path needs a deferred-action seam in `AppCore` and is tracked as an architecture follow-up.
- **Portable zip updates:** only the NSIS installer has the kill-before-replace hook. A user who replaces the portable binary while the old process runs still gets the version-skew 1 s no-op; document this in the release notes rather than adding a process-kill path to the portable artifact.
- **Winget PR merge:** external moderator review. The repository manifest and PR update are complete in Task 8, but `WINGET_LIVE` in `docs/index.html` flips only after merge and CDN indexing.

## Self-Review

- Spec coverage: every Session 123 finding maps to a task or the explicit follow-up list; no silent omission.
- Placeholder scan: no TBD/TODO; every code step contains the intended code or command.
- Type consistency: `ensure_background_flag` returns `Result<bool, String>` in both cfg branches; `ensure_single_instance(bool)` has one call site; `is_host_launch` / `is_background_launch` take `&[String]` in both producer and consumers.
- Ordering: Task 1 before Task 2 (function exists), Task 2 before Task 7 (runtime gate catches the semantics), Task 6 before Task 8 (tag version must exist).
