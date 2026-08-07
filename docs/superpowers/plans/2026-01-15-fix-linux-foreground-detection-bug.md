# Plan: Fix Linux Foreground Process Detection Bug

**Spec Reference**: [2026-01-15-fix-linux-foreground-detection-bug.md](../specs/2026-01-15-fix-linux-foreground-detection-bug.md)  
**Estimated Time**: 4 hours  
**Priority**: Critical (blocking Linux users from reliable blacklist functionality)

---

## Phase 1: Preparation (30 min)

### Task 1.1: Update Harness Documentation
- [ ] Add new feature to `feature_list.json`: `vol-012` - "Fix Linux foreground detection bug"
- [ ] Create entry in `claude-progress.md` documenting this fix initiative
- [ ] Status: `in_progress`

### Task 1.2: Review Current Implementation
- [ ] Audit existing `foreground_process()` implementations (Windows, macOS, Linux)
- [ ] Identify all call sites of `foreground_process()` in codebase
- [ ] Document current behavior vs expected behavior

---

## Phase 2: Dependency Management (30 min)

### Task 2.1: Add x11rb Dependency
- [ ] Edit `crates/volumectl/Cargo.toml`
- [ ] Add platform-specific dependency:
  ```toml
  [target.'cfg(target_os = "linux")'.dependencies]
  x11rb = { version = "0.13", features = ["allow-unsafe-code"] }
  ```
- [ ] Run `cargo check --target x86_64-unknown-linux-gnu` to verify compilation

### Task 2.2: Verify No Windows/macOS Impact
- [ ] Run `cargo check --target x86_64-pc-windows-msvc`
- [ ] Run `cargo check --target x86_64-apple-darwin`
- [ ] Confirm no dependency leakage

---

## Phase 3: Implementation (2 hours)

### Task 3.1: Implement X11 Helper Functions
- [ ] Add `get_active_window_x11()` function in `app.rs`
- [ ] Add `get_window_pid_x11()` function in `app.rs`
- [ ] Handle errors gracefully (return None on failure)
- [ ] Add appropriate `#[cfg(target_os = "linux")]` guards

### Task 3.2: Refactor foreground_process() for Linux
- [ ] Keep Method 1 (xdotool) - working correctly
- [ ] Keep Method 2 (xprop + wmctrl) - working correctly
- [ ] **REMOVE** broken Method 3 (/proc enumeration)
- [ ] **ADD** Method 3 (x11rb direct X11 query)
- [ ] Add logging at each fallback stage
- [ ] Ensure final fallback returns None (not wrong answer)

### Task 3.3: Code Quality
- [ ] Run `cargo fmt --all`
- [ ] Run `cargo clippy --target x86_64-unknown-linux-gnu`
- [ ] Address all warnings
- [ ] Add inline comments explaining each method's tradeoffs

---

## Phase 4: Testing (1 hour)

### Task 4.1: Unit Tests
- [ ] Create test module in `app.rs` with mocked X11 calls
- [ ] Test: `get_active_window_x11()` returns valid window ID
- [ ] Test: `get_window_pid_x11()` extracts correct PID
- [ ] Test: `foreground_process()` returns normalized name
- [ ] Test: All methods fail gracefully → returns None

### Task 4.2: Integration Tests
- [ ] Add test in `tests/` directory for Linux target
- [ ] Simulate different foreground apps (mocked)
- [ ] Verify blacklist matching logic end-to-end

### Task 4.3: Build Verification
- [ ] `cargo build --target x86_64-unknown-linux-gnu` succeeds
- [ ] `cargo test --target x86_64-unknown-linux-gnu` passes all tests
- [ ] No new warnings introduced

---

## Phase 5: Documentation & Wrap-up (30 min)

### Task 5.1: Update README
- [ ] Add Linux dependencies section:
  - Recommended: `xdotool`, `wmctrl`
  - Optional: X11 (usually pre-installed)
- [ ] Add troubleshooting for Wayland users
- [ ] Note that some features require X11 session

### Task 5.2: Update Harness
- [ ] Mark `vol-012` as `passing` in `feature_list.json`
- [ ] Add evidence to `claude-progress.md`
- [ ] Record test results and build artifacts

### Task 5.3: Final Verification
- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --all-targets` clean
- [ ] Git diff review - only intended changes
- [ ] Commit message follows convention

---

## Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Build success rate | 100% | cargo build on Linux target |
| Test pass rate | 100% | cargo test on Linux target |
| Code quality | 0 clippy warnings | cargo clippy |
| Formatting | Pass | cargo fmt --check |
| Documentation | Complete | README updated, spec linked |

---

## Rollback Triggers

If any of these occur, rollback immediately:
1. ❌ Windows or macOS builds break
2. ❌ Existing tests start failing
3. ❌ Performance regression (>10ms latency added)
4. ❌ Cannot reproduce fix in test environment

**Rollback Command**: `git revert HEAD` (after committing fix)

---

## Notes

- This fix is CRITICAL for Linux users - current implementation is actively harmful
- Prefer returning `None` over wrong answer - caller handles gracefully
- X11 connection overhead is acceptable (<1ms typical)
- Future enhancement: Add Wayland-native support via dbus/portal
