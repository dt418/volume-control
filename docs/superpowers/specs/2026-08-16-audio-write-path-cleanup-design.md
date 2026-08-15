# Audio Write-Path Cleanup Design

**Status:** Approved (Plan C of the review-findings decomposition)

**Goal:** Make unmute-on-volume-change a single policy in `AppCore` instead
of duplicating it in the Windows backend, and lock that policy with a
regression test.

**Background:** `WindowsAudio::set_volume` unmutes when the scalar is > 0
(`audio_windows.rs`), and `host_core::handle_action` also calls
`audio.set_mute(false)` after every successful positive volume write. Each
volume-up step therefore issues two WASAPI calls, and the "raising volume
unmutes" rule lives in two layers that can drift.

## Scope

`crates/volumectl/src/audio_windows.rs`,
`crates/volumectl/src/host_core.rs` (no change needed beyond confirming the
existing unmute paths), and `crates/volumectl/tests/host_core.rs`. No public
interface changes.

## Requirements

### R1 — Backend sets scalar only

- `audio_windows.rs` `AudioBackend::set_volume`: remove the
  `if v > 0 { let _ = self.set_mute(false); }` block and its comment. The
  backend's job is to write the scalar volume; unmute policy belongs to
  `AppCore`.
- `host_core.rs` keeps the existing `set_mute(false)` + `mute_restore_volume
  = None` calls in `SetVolumePercent`, `AdjustVolume`, and `ResetVolume` — no
  change.

### R2 — Regression tests for the policy

- In `crates/volumectl/tests/host_core.rs` (StubAudio does not auto-unmute,
  matching the new backend contract), add a test:

```rust
#[test]
fn positive_volume_writes_unmute_and_clear_restore() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink);
    core.handle_action(AppAction::ToggleMute);
    assert!(core.bootstrap().muted);

    core.handle_action(AppAction::AdjustVolume { delta_percent: 10 });
    assert!(!core.bootstrap().muted, "volume up must unmute");
    assert_eq!(core.bootstrap().volume_pct, 10);

    core.handle_action(AppAction::ToggleMute);
    core.handle_action(AppAction::SetVolumePercent { percent: 40 });
    assert!(!core.bootstrap().muted, "explicit set must unmute");

    core.handle_action(AppAction::ToggleMute);
    core.handle_action(AppAction::ResetVolume);
    assert!(!core.bootstrap().muted, "reset must unmute");
    assert_eq!(core.bootstrap().volume_pct, 50);
}
```

- The existing `volume_actions_cover_small_large_reset_clamping_and_mute`
  test continues to pass unchanged.

## Testing

- `cargo test -p volumectl --no-default-features --test host_core` passes
  including the new test.
- `cargo clippy --workspace --all-targets --no-default-features --
  -D warnings` and `cargo fmt --all --check` pass.
- E2E mixer spec (`verify-tauri-e2e.ps1 -Surface mixer`) stays green — the
  mute/unmute round-trip through the virtual audio endpoint exercises the
  same policy.

## Acceptance Criteria

- No duplicate unmute write on Windows: exactly one WASAPI `SetMute(false)`
  per positive volume write from the host policy.
- `StubAudio`/`E2eAudio` behavior unchanged (they never auto-unmuted, so the
  new backend contract matches their behavior).
- The new regression test fails if the host unmute policy is removed.

## Residual Risks

- Any future direct `WindowsAudio::set_volume` caller outside `AppCore` would
  no longer unmute; today the backend is only reachable through `AppCore`
  (SSOT). The test above documents the contract.
