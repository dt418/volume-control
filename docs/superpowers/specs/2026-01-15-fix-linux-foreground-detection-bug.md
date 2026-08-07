# Spec: Fix Linux Foreground Process Detection Bug

## Problem Statement

**Critical Bug**: The `foreground_process()` function for Linux (app.rs lines 258-271) has fundamentally broken logic that returns the FIRST process found in `/proc` enumeration instead of the ACTUALLY focused window's process.

### Current Broken Code (Method 3 Fallback):
```rust
// Method 3: Direct /proc enumeration as last resort
for entry in std::fs::read_dir("/proc").ok()?.flatten() {
    if let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() {
        let comm_path = format!("/proc/{}/comm", pid);
        if let Ok(comm) = std::fs::read_to_string(&comm_path) {
            let name = comm.trim().to_lowercase();
            return Some(crate::config::normalize_blacklist_entry(&name));  // ❌ BUG!
        }
    }
}
```

**Issue**: This returns an arbitrary process (typically PID 1 or another early process), NOT the foreground window. Blacklist matching becomes completely unreliable on Linux.

### Impact:
- Linux users experience random hotkey blocking/unblocking
- Blacklist feature is essentially non-functional
- Poor user experience, potential conflicts with system apps

## Root Cause Analysis

The fallback Method 3 was intended as a "last resort" but lacks any window-to-process mapping. Methods 1 (xdotool) and 2 (xprop+wmctrl) are correct but may fail if tools aren't installed. The fallback should either:
1. Use proper X11/Wayland APIs to get active window
2. Return None and let the caller handle it gracefully

## Technical Design

### Solution Architecture

**Option A: Add x11rb dependency (Recommended)**
- Use x11rb crate to query X11 `_NET_ACTIVE_WINDOW` directly
- Map window ID to PID via `_NET_WM_PID` property
- Read process name from `/proc/{pid}/comm`
- Pros: No external CLI dependencies, reliable, pure Rust
- Cons: Adds dependency, X11-only (Wayland needs different approach)

**Option B: Improve fallback with better detection**
- Remove Method 3 entirely (it's worse than useless)
- Return None if xdotool and xprop both fail
- Document required dependencies in README
- Pros: No new dependencies, honest about limitations
- Cons: Requires users to install tools

**Option C: Hybrid approach (Best)**
- Keep Methods 1 & 2 (xdotool, xprop+wmctrl)
- Replace Method 3 with x11rb-based direct X11 query
- Add Wayland support via dbus (optional, future)
- Graceful degradation: return None if all methods fail

### Implementation Details

#### New Dependency (Cargo.toml):
```toml
[target.'cfg(target_os = "linux")'.dependencies]
x11rb = { version = "0.13", features = ["allow-unsafe-code"] }
```

#### Fixed foreground_process() Logic:
```rust
#[cfg(target_os = "linux")]
fn foreground_process() -> Option<String> {
    use std::process::Command;
    
    // Method 1: Try xdotool first (most reliable if available)
    if let Ok(output) = Command::new("xdotool")
        .args(["getactivewindow", "--pid"])
        .output()
    {
        if output.status.success() {
            let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(pid) = pid_str.parse::<u32>() {
                if let Ok(comm) = std::fs::read_to_string(format!("/proc/{}/comm", pid)) {
                    let name = comm.trim().to_lowercase();
                    return Some(crate::config::normalize_blacklist_entry(&name));
                }
            }
        }
    }
    
    // Method 2: Fallback to xprop + wmctrl
    let output = Command::new("xprop")
        .args(["-root", "_NET_ACTIVE_WINDOW"])
        .output()
        .ok()?;
    
    if !output.status.success() {
        return None;
    }
    
    let window_id = String::from_utf8_lossy(&output.stdout);
    let window_id = window_id.split('#').nth(1)?.trim();
    
    let wmctrl_output = Command::new("wmctrl")
        .arg("-lp")
        .output()
        .ok()?;
    
    if wmctrl_output.status.success() {
        let lines = String::from_utf8_lossy(&wmctrl_output.stdout);
        for line in lines.lines() {
            if line.contains(window_id) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    if let Ok(pid) = parts[1].parse::<u32>() {
                        if let Ok(comm) = std::fs::read_to_string(format!("/proc/{}/comm", pid)) {
                            let name = comm.trim().to_lowercase();
                            return Some(crate::config::normalize_blacklist_entry(&name));
                        }
                    }
                }
            }
        }
    }
    
    // Method 3: Direct X11 query via x11rb (no CLI deps needed)
    // This replaces the broken /proc enumeration
    if let Ok(active_window) = get_active_window_x11() {
        if let Some(pid) = get_window_pid_x11(active_window) {
            if let Ok(comm) = std::fs::read_to_string(format!("/proc/{}/comm", pid)) {
                let name = comm.trim().to_lowercase();
                return Some(crate::config::normalize_blacklist_entry(&name));
            }
        }
    }
    
    // All methods failed - return None (better than wrong answer)
    log::warn!("Could not determine foreground process on Linux");
    None
}

fn get_active_window_x11() -> Option<u32> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;
    
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    
    let prop = conn
        .get_property(false, root, AtomEnum::_NET_ACTIVE_WINDOW, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?;
    
    if prop.value.is_empty() {
        return None;
    }
    
    Some(u32::from_ne_bytes([prop.value[0], prop.value[1], prop.value[2], prop.value[3]]))
}

fn get_window_pid_x11(window: u32) -> Option<u32> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;
    
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let screen = &conn.setup().roots[screen_num];
    
    let prop = conn
        .get_property(false, window, AtomEnum::_NET_WM_PID, AtomEnum::CARDINAL, 0, 1)
        .ok()?
        .reply()
        .ok()?;
    
    if prop.value.len() < 4 {
        return None;
    }
    
    Some(u32::from_ne_bytes([prop.value[0], prop.value[1], prop.value[2], prop.value[3]]))
}
```

### Success Criteria

1. **Functional Correctness**:
   - [ ] `foreground_process()` returns the ACTUAL foreground application name on Linux
   - [ ] Blacklist matching works reliably (tested with known apps: firefox, code, gnome-terminal)
   - [ ] Returns None gracefully when no method succeeds (better than wrong answer)

2. **Performance**:
   - [ ] No noticeable latency in hotkey processing (<10ms overhead)
   - [ ] X11 connection cached or reused efficiently

3. **Compatibility**:
   - [ ] Works on X11 sessions (Ubuntu, Fedora, etc.)
   - [ ] Gracefully degrades on Wayland (logs warning, returns None)
   - [ ] No breaking changes to existing config format

4. **Testing**:
   - [ ] Unit tests for each method (mocked where needed)
   - [ ] Integration test: run app on Linux, verify blacklist blocks correctly
   - [ ] Manual verification on real Linux system

5. **Documentation**:
   - [ ] README updated with Linux dependencies (xdotool, wmctrl recommended)
   - [ ] Troubleshooting section for Wayland users

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| x11rb adds compile time | Low | Low | Only on Linux target, cached builds |
| Wayland incompatibility | Medium | Medium | Document limitation, return None gracefully |
| X11 connection failures | Low | Low | Fallback chain handles failures |
| Breaking existing configs | None | N/A | No config format changes |

## Rollback Plan

If issues arise:
1. Revert to previous version (Method 3 removed, only Methods 1&2)
2. Users can install xdotool/wmctrl as workaround
3. File follow-up issue for Wayland-native solution

## References

- [x11rb documentation](https://docs.rs/x11rb/latest/x11rb/)
- [EWMH Specification - _NET_ACTIVE_WINDOW](https://specifications.freedesktop.org/wm-spec/1.3/ar01s03.html)
- [Linux /proc filesystem](https://man7.org/linux/man-pages/man5/proc.5.html)
