# Cross-Platform Blacklist Optimization Spec

**Date:** 2026-01-15  
**Author:** AI Assistant  
**Status:** Approved for Implementation

## Executive Summary

This spec addresses critical gaps in the VolumeControl application's blacklist functionality to ensure consistent behavior across Windows, macOS, and Linux platforms. The current implementation has platform-specific limitations that prevent effective hotkey suppression on non-Windows systems.

## Problem Statement

### Current Issues

1. **Blacklist Inconsistency**: The `normalize()` function in `config.rs` filters blacklist entries to only accept `.exe` extensions (line 385), making it impossible to add macOS (`.app`) or Linux (no extension) applications.

2. **Foreground Process Detection Gaps**: 
   - macOS: Uses AppleScript but doesn't normalize app names properly
   - Linux: Depends on external tools (`xprop`, `wmctrl`, `xwininfo`) which may not be installed
   - No graceful fallback when detection fails

3. **System Key Conflict Risks**:
   - Default `CtrlAlt` modifier can conflict with system shortcuts
   - No user guidance on platform-specific conflicts
   - Missing warnings for potentially problematic configurations

4. **Performance Concerns**:
   - `foreground_process()` is called on every hotkey event
   - No caching mechanism for process name lookups
   - Blocking system calls without timeout handling

## Goals

### Primary Objectives

1. **Cross-Platform Blacklist Support**: Enable blacklist functionality on all three platforms with appropriate naming conventions
2. **Robust Foreground Detection**: Implement reliable process detection with graceful degradation
3. **Conflict Prevention**: Add proactive warnings and recommendations for modifier selection
4. **Performance Optimization**: Cache foreground process information with intelligent invalidation

### Success Criteria

- ✅ Blacklist works consistently on Windows, macOS, and Linux
- ✅ App detects and warns about potential key conflicts
- ✅ Hotkey response time < 10ms even with blacklist checking
- ✅ Graceful operation when system tools are unavailable
- ✅ All existing tests pass + new cross-platform tests added

## Technical Design

### 1. Platform-Aware Blacklist Normalization

**Current Behavior:**
```rust
cfg.blacklist = cfg.blacklist
    .iter()
    .map(|s| s.trim().to_lowercase())
    .filter(|s| s.ends_with(".exe"))  // ❌ Windows-only!
    .collect();
```

**Proposed Behavior:**
```rust
cfg.blacklist = cfg.blacklist
    .iter()
    .map(|s| normalize_blacklist_entry(s))
    .collect();

fn normalize_blacklist_entry(entry: &str) -> String {
    let entry = entry.trim().to_lowercase();
    #[cfg(target_os = "windows")]
    return if entry.ends_with(".exe") { entry } else { format!("{}.exe", entry) };
    
    #[cfg(target_os = "macos")]
    return if entry.ends_with(".app") { entry } else { format!("{}.app", entry) };
    
    #[cfg(target_os = "linux")]
    return entry; // No extension convention on Linux
}
```

### 2. Enhanced Foreground Process Detection

#### macOS Improvements
- Use NSWorkspace API via objc crate for faster, more reliable detection
- Fallback to AppleScript if native API fails
- Cache results for 100ms to reduce IPC overhead

#### Linux Improvements
- Primary: Read from `/proc/$(xdotool getactivewindow --pid)/comm`
- Fallback 1: xprop + wmctrl combination
- Fallback 2: Direct /proc enumeration by window ID
- Graceful no-op when X11/Wayland tools unavailable

### 3. Smart Caching Layer

```rust
struct ForegroundCache {
    last_check: Instant,
    cached_name: Option<String>,
    cache_duration: Duration, // 50-100ms
}

impl ForegroundCache {
    fn get_or_refresh(&mut self) -> Option<String> {
        if self.last_check.elapsed() < self.cache_duration {
            return self.cached_name.clone();
        }
        self.cached_name = foreground_process_raw();
        self.last_check = Instant::now();
        self.cached_name.clone()
    }
}
```

### 4. Conflict Detection & Warnings

```rust
pub struct ModifierConflictInfo {
    pub modifier: HotkeyModifier,
    pub platform_conflicts: Vec<&'static str>,
    pub risk_level: ConflictRisk, // Low, Medium, High
    pub recommendation: &'static str,
}

pub fn check_modifier_conflicts(modifier: HotkeyModifier) -> ModifierConflictInfo {
    // Platform-specific conflict database
}
```

### 5. Recommended Blacklist Entries by Platform

Extend `recommended_blacklist()` to return platform-appropriate entries:

```rust
pub fn recommended_blacklist(modifier: HotkeyModifier) -> Vec<String> {
    let list: &[&str] = match modifier {
        HotkeyModifier::CtrlAlt | HotkeyModifier::CapsLock => &[],
        HotkeyModifier::Alt => get_alt_conflict_apps(),
        HotkeyModifier::Ctrl => get_ctrl_conflict_apps(),
    };
    
    // Platform-aware normalization
    list.iter()
        .map(|s| normalize_for_platform(s))
        .collect()
}
```

## Implementation Plan

### Phase 1: Foundation (Priority: Critical)
1. Fix `normalize()` to accept platform-appropriate extensions
2. Update `recommended_blacklist()` with platform-specific entries
3. Add `is_blacklisted()` pattern matching for partial names

### Phase 2: Foreground Detection (Priority: High)
1. Improve macOS detection with NSWorkspace
2. Enhance Linux detection with better fallbacks
3. Add feature detection for required tools

### Phase 3: Performance (Priority: Medium)
1. Implement foreground process caching
2. Add async-friendly design for future UI integration
3. Profile and optimize hot path

### Phase 4: User Experience (Priority: Medium)
1. Add conflict warning system
2. Improve error messages for missing tools
3. Add help documentation for each platform

## Testing Strategy

### Unit Tests
- Blacklist normalization per platform
- Pattern matching logic
- Cache invalidation timing
- Conflict detection accuracy

### Integration Tests
- End-to-end hotkey suppression
- Multi-application switching scenarios
- Tool absence simulation (Linux)

### Manual Testing Checklist
- [ ] Windows: VSCode, Chrome, IDE blacklisting
- [ ] macOS: Safari, Xcode, VS Code blacklisting
- [ ] Linux: Firefox, GNOME Terminal, Vim blacklisting
- [ ] Verify no conflicts with system shortcuts
- [ ] Measure hotkey latency with/without cache

## Risk Mitigation

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| Breaking existing configs | High | Low | Backward-compatible normalization |
| Performance regression | Medium | Low | Caching + benchmarking |
| False positives in detection | Medium | Medium | Conservative matching + user override |
| Missing dependencies on Linux | Low | High | Graceful degradation |

## Migration Path

Existing users will experience:
1. Automatic config migration on first load
2. Warning if old `.exe`-only entries exist on non-Windows
3. Recommendation to update blacklist based on current modifier

## Acceptance Criteria

- [ ] All unit tests pass on Windows, macOS, and Linux
- [ ] Blacklist correctly blocks hotkeys in target apps on all platforms
- [ ] No measurable performance degradation (< 1ms overhead)
- [ ] Clear user guidance for platform-specific setup
- [ ] Documentation updated with platform notes

## Appendix A: Platform-Specific Application Names

### Windows (`.exe`)
- `code.exe` - Visual Studio Code
- `chrome.exe` - Google Chrome
- `idea64.exe` - IntelliJ IDEA

### macOS (`.app`)
- `visual studio code.app`
- `google chrome.app`
- `intellij idea.app`
- `safari.app`
- `xcode.app`

### Linux (no extension)
- `code`
- `google-chrome`
- `firefox`
- `gnome-terminal`
- `alacritty`

---

**Next Steps:**
1. Review and approve this spec
2. Create implementation tasks
3. Set up CI testing for all three platforms
4. Begin Phase 1 implementation
