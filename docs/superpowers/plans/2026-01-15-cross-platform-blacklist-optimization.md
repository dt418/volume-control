# Cross-Platform Blacklist Optimization Plan

**Date:** 2026-01-15  
**Related Spec:** `2026-01-15-cross-platform-blacklist-optimization.md`  
**Status:** Ready for Implementation

## Overview

This plan breaks down the implementation of cross-platform blacklist optimization into discrete, testable tasks. Each task includes acceptance criteria and estimated effort.

## Phase 1: Foundation Fixes (Critical)

### Task 1.1: Fix Blacklist Normalization
**File:** `crates/volumectl/src/config.rs`  
**Effort:** 1 hour  
**Priority:** P0

**Changes:**
- Modify `normalize()` to accept platform-appropriate extensions
- Remove `.exe`-only filter
- Add platform-specific normalization helper

**Acceptance Criteria:**
- [ ] Windows configs preserve `.exe` entries
- [ ] macOS configs accept `.app` entries
- [ ] Linux configs accept extensionless entries
- [ ] Existing Windows configs remain valid
- [ ] Unit tests for all three platforms

### Task 1.2: Update Recommended Blacklist
**File:** `crates/volumectl/src/config.rs`  
**Effort:** 2 hours  
**Priority:** P0

**Changes:**
- Extend `recommended_blacklist()` with platform-specific app lists
- Add macOS app names (`.app` suffix)
- Add Linux binary names (no suffix)
- Keep Windows list unchanged

**Acceptance Criteria:**
- [ ] macOS list includes Safari, Chrome, VS Code, Xcode
- [ ] Linux list includes firefox, code, gnome-terminal
- [ ] All entries properly normalized for their platform
- [ ] Tests verify correct entries per platform

### Task 1.3: Improve Blacklist Matching
**File:** `crates/volumectl/src/config.rs`  
**Effort:** 1 hour  
**Priority:** P1

**Changes:**
- Enhance `is_blacklisted()` to support partial matching
- Support matching with/without extension
- Add case-insensitive comparison

**Acceptance Criteria:**
- [ ] `code` matches `code.exe` on Windows
- [ ] `visual studio code` matches `visual studio code.app` on macOS
- [ ] Partial name matching works reliably
- [ ] Performance impact < 0.1ms per check

## Phase 2: Foreground Detection Improvements

### Task 2.1: Enhance macOS Detection
**File:** `crates/volumectl/src/app.rs`  
**Effort:** 3 hours  
**Priority:** P1

**Changes:**
- Add NSWorkspace-based detection using `objc` crate
- Improve AppleScript fallback
- Normalize app names consistently (with `.app` suffix)
- Add timeout handling (max 50ms)

**Dependencies:**
- Add `objc = "0.2"` to Cargo.toml (macOS only)
- May need `block = "0.1"` as well

**Acceptance Criteria:**
- [ ] Detects running apps accurately
- [ ] Returns `.app` suffixed names
- [ ] Handles permission errors gracefully
- [ ] Times out after 50ms if unresponsive
- [ ] Works with both Intel and Apple Silicon

### Task 2.2: Enhance Linux Detection
**File:** `crates/volumectl/src/app.rs`  
**Effort:** 3 hours  
**Priority:** P1

**Changes:**
- Primary: Use xdotool if available (`xdotool getactivewindow --pid`)
- Fallback 1: xprop + wmctrl combination
- Fallback 2: Direct /proc enumeration
- Graceful no-op when all methods fail

**Acceptance Criteria:**
- [ ] Works with X11 sessions
- [ ] Works with Wayland (via xwayland)
- [ ] Gracefully degrades when tools missing
- [ ] Logs warnings for missing dependencies
- [ ] Returns process names without extension

### Task 2.3: Add Tool Detection Helper
**File:** `crates/volumectl/src/app.rs` (new module)  
**Effort:** 1 hour  
**Priority:** P2

**Changes:**
- Create `linux_deps.rs` module
- Detect availability of xdotool, xprop, wmctrl
- Provide feature flags for conditional logic
- Add user-facing diagnostic command

**Acceptance Criteria:**
- [ ] Detects all required tools
- [ ] Provides clear error messages
- [ ] Suggests installation commands
- [ ] CLI command: `volumectl diagnose`

## Phase 3: Performance Optimization

### Task 3.1: Implement Foreground Cache
**File:** `crates/volumectl/src/app.rs`  
**Effort:** 2 hours  
**Priority:** P1

**Changes:**
- Add `ForegroundCache` struct
- Cache duration: 100ms (configurable)
- Thread-safe using `Arc<Mutex<>>`
- Invalidate on hotkey release events

**Acceptance Criteria:**
- [ ] Reduces foreground_process() calls by 90%+
- [ ] Cache hit latency < 0.01ms
- [ ] Cache miss latency acceptable (< 50ms)
- [ ] No race conditions in multi-threaded context
- [ ] Benchmarks show improvement

### Task 3.2: Add Performance Benchmarks
**File:** `crates/volumectl/benches/blacklist_bench.rs`  
**Effort:** 1 hour  
**Priority:** P2

**Changes:**
- Benchmark blacklist checking
- Benchmark foreground detection
- Benchmark cache performance
- Compare before/after metrics

**Acceptance Criteria:**
- [ ] All benchmarks run with `cargo bench`
- [ ] Baseline metrics recorded
- [ ] Regression detection in CI
- [ ] Documentation of expected performance

## Phase 4: User Experience

### Task 4.1: Add Conflict Detection
**File:** `crates/volumectl/src/config.rs` (new module)  
**Effort:** 2 hours  
**Priority:** P2

**Changes:**
- Create `conflicts.rs` module
- Define known system shortcuts per platform
- Warn users about potential conflicts
- Suggest alternative modifiers

**Acceptance Criteria:**
- [ ] Detects Ctrl+Alt conflicts on Linux (workspace switching)
- [ ] Detects Cmd conflicts on macOS (system shortcuts)
- [ ] Warns about Ctrl conflicts in browsers/editors
- [ ] Provides actionable recommendations

### Task 4.2: Improve Settings UI Warnings
**File:** `crates/volumectl/src/settings.rs`  
**Effort:** 2 hours  
**Priority:** P2

**Changes:**
- Show conflict warnings in modifier dropdown
- Display platform-specific help text
- Add tooltip with common conflicts
- Highlight recommended modifiers

**Acceptance Criteria:**
- [ ] Users see warnings before enabling risky modifiers
- [ ] Help text is platform-appropriate
- [ ] Recommendations are clear and actionable
- [ ] No false positives

### Task 4.3: Update Documentation
**Files:** `README.md`, `README.vi.md`, `docs/`  
**Effort:** 2 hours  
**Priority:** P1

**Changes:**
- Add platform-specific setup instructions
- Document blacklist format for each OS
- List common applications per platform
- Troubleshooting guide for detection issues

**Acceptance Criteria:**
- [ ] README updated with platform notes
- [ ] Vietnamese translation updated
- [ ] Blacklist examples for all platforms
- [ ] Troubleshooting section added

## Phase 5: Testing & Validation

### Task 5.1: Add Unit Tests
**File:** `crates/volumectl/src/config.rs`, `app.rs`  
**Effort:** 3 hours  
**Priority:** P0

**Test Coverage:**
- Blacklist normalization (all platforms)
- Pattern matching logic
- Cache behavior
- Conflict detection
- Foreground detection mocks

**Acceptance Criteria:**
- [ ] 100% coverage of new code
- [ ] All existing tests still pass
- [ ] Platform-specific tests marked correctly
- [ ] Tests run in CI for all targets

### Task 5.2: Add Integration Tests
**File:** `crates/volumectl/tests/`  
**Effort:** 2 hours  
**Priority:** P1

**Test Scenarios:**
- Hotkey suppression in blacklisted apps
- Multi-app switching behavior
- Cache coherency under load
- Error handling when tools missing

**Acceptance Criteria:**
- [ ] End-to-end hotkey flow tested
- [ ] Simulated app switching works
- [ ] Error paths covered
- [ ] Manual test checklist created

### Task 5.3: CI/CD Updates
**File:** `.github/workflows/`, `Cargo.toml`  
**Effort:** 1 hour  
**Priority:** P1

**Changes:**
- Add macOS build/test to CI
- Add Linux build/test to CI
- Include integration test runners
- Add performance regression checks

**Acceptance Criteria:**
- [ ] CI builds for all three platforms
- [ ] Tests run on each PR
- [ ] Performance benchmarks tracked
- [ ] Build artifacts generated

## Implementation Timeline

| Phase | Tasks | Estimated Time | Dependencies |
|-------|-------|----------------|--------------|
| Phase 1 | 1.1, 1.2, 1.3 | 4 hours | None |
| Phase 2 | 2.1, 2.2, 2.3 | 7 hours | Phase 1 |
| Phase 3 | 3.1, 3.2 | 3 hours | Phase 2 |
| Phase 4 | 4.1, 4.2, 4.3 | 6 hours | Phase 1 |
| Phase 5 | 5.1, 5.2, 5.3 | 6 hours | All phases |
| **Total** | **11 tasks** | **26 hours** | - |

## Risk Register

| ID | Risk | Impact | Probability | Mitigation |
|----|------|--------|-------------|------------|
| R1 | Breaking existing Windows configs | High | Low | Backward-compatible migration |
| R2 | objc crate compatibility issues | Medium | Low | Keep AppleScript fallback |
| R3 | Linux tool fragmentation | Medium | High | Multiple fallbacks + graceful degradation |
| R4 | Performance regression | Medium | Low | Caching + benchmarks |
| R5 | False positive blacklist matches | Low | Medium | Conservative matching + user override |

## Rollout Strategy

### Stage 1: Internal Testing
- Developer testing on all three platforms
- Dogfood with team members
- Collect feedback on false positives/negatives

### Stage 2: Beta Release
- Release as beta feature
- Gather community feedback
- Monitor crash reports and issues

### Stage 3: General Availability
- Full release with documentation
- Migration guide for existing users
- Support channel ready for questions

## Success Metrics

- **Functional**: Blacklist works on all 3 platforms (manual verification)
- **Performance**: < 1ms overhead for blacklist checking (benchmarks)
- **Adoption**: 80% of users on non-Windows platforms use blacklist (telemetry if available)
- **Satisfaction**: < 5% issue rate related to hotkey conflicts (GitHub issues)

## Post-Launch Tasks

- [ ] Monitor GitHub issues for platform-specific problems
- [ ] Collect user feedback on conflict warnings
- [ ] Update recommended blacklist based on user reports
- [ ] Consider adding user-contributed blacklist repository
- [ ] Evaluate adding GUI blacklist manager

---

**Approval:**
- [ ] Spec reviewed and approved
- [ ] Resources allocated
- [ ] Timeline agreed upon
- [ ] Risk mitigation planned

**Kickoff:** Ready to begin Phase 1 implementation
