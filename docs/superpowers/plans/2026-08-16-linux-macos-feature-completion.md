# Complete Missing Linux/macOS Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the three remaining platform-table gaps on macOS and Linux —
system tray, overlay HUD host integration, and Linux per-app audio — without
changing the verified Windows native surfaces.

**Architecture:** Phase A adds a Tauri-managed tray on macOS/Linux that reuses
the existing `TrayCommand` id mapping. Phase B adds a `window-overlay`
webview HUD surface driven by the existing `EventSink::overlay` notification
with a cancellable auto-hide timer. Phase C implements `SessionsSource` on
Linux with a direct `libpulse-sys` threaded-mainloop connection over Pulse
sink-inputs; macOS keeps the honest "not supported" boundary.

**Tech Stack:** Rust 1.82 workspace, Tauri 2 (`tray-icon`, `macos-private-api`
features), muda menu ids, WebKitGTK/WebView2 webviews, React 19 + Vite +
Vitest, WebdriverIO/Tauri E2E, libpulse-sys 1.23.

## Global Constraints

- Windows native overlay/tray and the Windows WASAPI session path stay
  byte-for-byte behaviorally unchanged; `native_win32.rs` is not edited.
- macOS per-app audio is **not supported** (no public API since 10.14); UI
  copy and README rows state this truthfully — no private/hacky APIs.
- Tray creation failure is non-fatal: log and keep the host alive (same
  pattern as `UnavailableAudio`).
- Menu ids are the single source of truth: `mute`, `reset`, `mixer`,
  `settings`, `help`, `reload`, `edit`, `exit` (existing `TrayCommand` ids).
- Every phase updates `feature_list.json` AND `claude-progress.md` in the
  same change set; README/README.vi rows change only with the evidence.
- TDD: every production behavior starts with a failing test.
- No changes to `init.sh`, `.github/workflows/release.yml`, or the Windows
  native surfaces.
- The repository gate stays: `cargo fmt --all --check`, `git diff --check`,
  `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`,
  `cargo test --workspace --no-default-features`, records guard.
- The records guard requires `feature_list.json` + `claude-progress.md`
  updates in the **same commit** as every code change (the pre-commit hook
  runs `check-records.sh --staged`); Tasks 3/8/11 are docs-only commits
  (README/checklist are records-exempt).

---

## Phase A — System tray on macOS and Linux

### Task 1: Cross-platform TrayCommand and shared tray icon

**Files:**
- Create: `crates/volumectl/src/tray_common.rs`
- Modify: `crates/volumectl/src/tray.rs` (remove enum + icon fn; import from
  `tray_common`)
- Modify: `crates/volumectl/src/lib.rs` (add `pub mod tray_common;`)
- Modify: `crates/volumectl/src/host_core.rs` (un-gate
  `tray_command_to_action`)

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces:
  - `pub enum TrayCommand { ToggleMute, Reset50, OpenMixer, Help, Settings,
    EditConfig, ReloadConfig, Exit }`
  - `impl TrayCommand { pub fn from_menu_id(id: &str) -> Option<Self> }`
  - `pub fn tray_icon_rgba() -> Vec<u8>` (32×32 RGBA speaker glyph)

- [ ] **Step 1: Write the failing cross-platform tests**

Create `crates/volumectl/src/tray_common.rs` with the enum, `from_menu_id`,
`tray_icon_rgba`, and tests moved verbatim from `tray.rs` (the
`every_native_menu_id_decodes_to_a_command` case list plus a 32×32 RGBA
assertion):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_native_menu_id_decodes_to_a_command() {
        let cases = [
            ("mute", TrayCommand::ToggleMute),
            ("reset", TrayCommand::Reset50),
            ("mixer", TrayCommand::OpenMixer),
            ("settings", TrayCommand::Settings),
            ("help", TrayCommand::Help),
            ("reload", TrayCommand::ReloadConfig),
            ("edit", TrayCommand::EditConfig),
            ("exit", TrayCommand::Exit),
        ];
        for (id, expected) in cases {
            assert_eq!(TrayCommand::from_menu_id(id), Some(expected));
        }
        assert_eq!(TrayCommand::from_menu_id("volume"), None);
        assert_eq!(TrayCommand::from_menu_id("unknown"), None);
    }

    #[test]
    fn tray_icon_is_32x32_rgba() {
        let px = tray_icon_rgba();
        assert_eq!(px.len(), 32 * 32 * 4);
        // At least one opaque blue speaker pixel and one transparent pixel.
        assert!(px.chunks_exact(4).any(|p| p[3] == 255));
        assert!(px.chunks_exact(4).any(|p| p[3] == 0));
    }
}
```

Add `pub mod tray_common;` to `crates/volumectl/src/lib.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p volumectl --lib tray_common --no-default-features`

Expected: FAIL — module not found / symbols not defined.

- [ ] **Step 3: Move the implementation**

Copy `TrayCommand`, `from_menu_id`, and `tray_icon_rgba` (with its doc
comments) from `crates/volumectl/src/tray.rs` into `tray_common.rs`. In
`tray.rs` replace them with:

```rust
use crate::tray_common::{tray_icon_rgba, TrayCommand};
```

and delete the old enum/icon/tests from `tray.rs` (its `Tray::poll` tests and
`Tray` struct stay).

- [ ] **Step 4: Un-gate the action mapping**

In `crates/volumectl/src/host_core.rs`:

```rust
use crate::tray_common::TrayCommand;
```

Remove `#[cfg(target_os = "windows")]` from `tray_command_to_action` (the fn
body is unchanged) and remove the `#[cfg(target_os = "windows")]` attribute
from its unit test (`tray_command_to_action_maps_all_commands`).

- [ ] **Step 5: Run the tests and the suite**

Run:

```bash
cargo test -p volumectl --lib tray --no-default-features
cargo test -p volumectl --lib --no-default-features
```

Expected: all pass, including the previously Windows-gated mapping test now
running on every platform.

- [ ] **Step 6: Commit**

```bash
git add crates/volumectl/src/tray_common.rs crates/volumectl/src/tray.rs crates/volumectl/src/lib.rs crates/volumectl/src/host_core.rs feature_list.json claude-progress.md
git commit -m "refactor: share tray commands and icon cross-platform"
```

The records update in this commit: `feature_list.json` gets the `vol-076`
entry (status `in_progress`, verification = the tests above, evidence =
local run output) and `claude-progress.md` gets the session entry — the
guard's required pair for every code change.

---

### Task 2: TauriTray for macOS and Linux

**Files:**
- Create: `src-tauri/src/tauri_tray.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/events_sink.rs`

**Interfaces:**
- Consumes: `volumectl_lib::tray_common::{TrayCommand, tray_icon_rgba}`,
  `volumectl_lib::audio::VolumeState`,
  `volumectl_lib::host_core::{AppCore, tray_command_to_action}`.
- Produces:
  - `pub struct TauriTray { tray: tauri::tray::TrayIcon, vol_label:
    tauri::menu::MenuItem, mute_item: tauri::menu::CheckMenuItem }`
  - `impl TauriTray { pub fn create(app: &tauri::AppHandle) ->
    Result<Self, String>; pub fn set_volume(&self, state: &VolumeState);
    pub fn show_menu(&self); }`
  - `fn dispatch_tray_command(app: &tauri::AppHandle,
    command: TrayCommand)` in `lib.rs` (shared by Windows global handler and
    the non-Windows tray callback).

- [ ] **Step 1: Write the failing module + wiring compile check**

Create `src-tauri/src/tauri_tray.rs` with the struct definition and an empty
`create` that returns `Err("not implemented".into())`, then add to
`src-tauri/src/lib.rs`:

```rust
#[cfg(not(target_os = "windows"))]
mod tauri_tray;
```

Run: `cargo check -p volumecontrol-tauri --no-default-features`

Expected: compiles (the module is cfg'd out on Windows; on Linux/macOS CI the
next steps fill it in).

- [ ] **Step 2: Run the TauriTray unit test (fails)**

Add to `tauri_tray.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tauri::menu::MenuId;

    #[test]
    fn menu_ids_match_the_shared_tray_command_mapping() {
        // Building a real tray needs a running app; assert the id contract
        // statically so the two sources cannot drift.
        for id in ["mute", "reset", "mixer", "settings", "help", "reload", "edit", "exit"] {
            assert!(
                TrayCommand::from_menu_id(id).is_some(),
                "tray menu id {id:?} must map to a TrayCommand"
            );
            let _ = MenuId::new(id.to_string());
        }
    }
}
```

Run: `cargo test -p volumecontrol-tauri --no-default-features`

Expected: FAIL to compile or run until `TrayCommand` is imported — write the
import line first (`use volumectl_lib::tray_common::TrayCommand;`) so the
failure is the missing menu construction, not the import.

- [ ] **Step 3: Implement create/set_volume/show_menu**

Replace the stub with the full implementation (Tauri v2 menu + tray APIs,
verified against the v2 docs; the icon is the shared generated glyph):

```rust
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager};

use volumectl_lib::audio::VolumeState;
use volumectl_lib::tray_common::{tray_icon_rgba, TrayCommand};

pub struct TauriTray {
    tray: TrayIcon,
    vol_label: MenuItem<tauri::Wry>,
    mute_item: CheckMenuItem<tauri::Wry>,
}

impl TauriTray {
    pub fn create(app: &AppHandle) -> Result<Self, String> {
        let vol_label = MenuItem::with_id(app, "volume", "VolumeControl — --", false, None::<&str>)
            .map_err(|e| e.to_string())?;
        let mute_item =
            CheckMenuItem::with_id(app, "mute", "Mute", true, false, None::<&str>)
                .map_err(|e| e.to_string())?;
        let reset = MenuItem::with_id(app, "reset", "Reset to 50%", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let mixer = MenuItem::with_id(app, "mixer", "Open mixer", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let help = MenuItem::with_id(app, "help", "Help", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let reload = MenuItem::with_id(app, "reload", "Reload configuration", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let edit = MenuItem::with_id(app, "edit", "Open config file", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let exit = MenuItem::with_id(app, "exit", "Exit VolumeControl", true, None::<&str>)
            .map_err(|e| e.to_string())?;

        let sep1 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
        let sep2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
        let sep3 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;

        let menu = Menu::with_items(
            app,
            &[
                &vol_label, &sep1, &mute_item, &reset, &mixer, &sep2, &settings, &help, &reload,
                &edit, &sep3, &exit,
            ],
        )
        .map_err(|e| e.to_string())?;

        let icon = tauri::image::Image::new_owned(tray_icon_rgba(), 32, 32)
            .map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::with_id("main")
            .icon(icon)
            .menu(&menu)
            .tooltip("VolumeControl")
            .show_menu_on_left_click(true)
            .on_menu_event(|app, event| {
                let Some(command) = TrayCommand::from_menu_id(event.id().as_ref()) else {
                    return;
                };
                super::dispatch_tray_command(app, command);
            })
            .build(app)
            .map_err(|e| e.to_string())?;

        Ok(Self {
            tray,
            vol_label,
            mute_item,
        })
    }

    pub fn set_volume(&self, state: &VolumeState) {
        let _ = self
            .vol_label
            .set_text(format!("VolumeControl — {}%", state.percent()));
        let _ = self.mute_item.set_checked(state.muted);
        // Tooltips are unsupported by the Linux appindicator backend; ignore.
        if let Err(e) = self.tray.set_tooltip(Some(&format!(
            "VolumeControl — {}%",
            state.percent()
        ))) {
            log::debug!("tray tooltip update unsupported: {e}");
        }
    }

    pub fn show_menu(&self) {
        // The Linux appindicator backend shows the menu on click; popping it
        // programmatically is Windows/macOS-only.
        if let Err(e) = self.tray.show_menu() {
            log::debug!("tray show_menu unsupported here: {e}");
        }
    }
}
```

- [ ] **Step 4: Wire dispatch + setup + sink**

In `src-tauri/src/lib.rs`:

```rust
/// Shared tray-command dispatch (Windows global handler and the
/// macOS/Linux tray callback both route here).
fn dispatch_tray_command(app: &tauri::AppHandle, command: TrayCommand) {
    let Some(shared) = app.try_state::<Arc<Mutex<AppCore>>>() else {
        log::warn!("tray command {:?} received before AppCore was managed", command);
        return;
    };
    log::debug!("tray command: {command:?}");
    let mut core = shared.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    core.handle_action(tray_command_to_action(command));
}
```

Leave `install_menu_event_handler` **Windows-only and unchanged in behavior**
(its closure now delegates to `dispatch_tray_command(app, command)` after
`TrayCommand::from_menu_id(event.id().as_ref())`). macOS/Linux do NOT install
the global handler: their tray menu events arrive at the
`TrayIconBuilder::on_menu_event` callback registered in `TauriTray::create`
(Task 2 Step 3), which calls the same `dispatch_tray_command` — one handler
per platform, so menu events can never dispatch twice. In `run()` setup,
after `app.manage(WindowManager...)`, add:

```rust
#[cfg(not(target_os = "windows"))]
match tauri_tray::TauriTray::create(&handle) {
    Ok(tray) => {
        app.manage(tray);
        log::info!("tray created");
    }
    Err(error) => log::warn!("tray unavailable; keeping host alive: {error}"),
}
```

In `src-tauri/src/events_sink.rs`:

```rust
#[cfg(not(target_os = "windows"))]
use crate::tauri_tray::TauriTray;
```

Extend `volume()` with:

```rust
#[cfg(not(target_os = "windows"))]
if let Some(tray) = self.app.try_state::<TauriTray>() {
    tray.set_volume(&VolumeState { volume: pct as f32 / 100.0, muted });
}
```

Replace the `show_tray_menu()` non-Windows no-op with:

```rust
#[cfg(not(target_os = "windows"))]
if let Some(tray) = self.app.try_state::<TauriTray>() {
    tray.show_menu();
} else {
    log::debug!("tray menu unavailable on this platform");
}
```

Also move `use tauri::Manager;` inside `events_sink.rs` (already imported)
so `try_state` resolves.

- [ ] **Step 5: Build + test**

Run:

```bash
cargo check -p volumecontrol-tauri --no-default-features
cargo test -p volumecontrol-tauri --no-default-features
```

Expected: compiles on Windows (tray module cfg'd out, existing handler
delegates to `dispatch_tray_command`) and the id-contract test passes.

- [ ] **Step 6: Linux-gnu cross-check**

Run: `cargo check -p volumectl --target x86_64-unknown-linux-gnu --no-default-features`

Expected: clean (verifies the un-gated `tray_command_to_action` compiles for
Linux without the tray-icon deps). The full `src-tauri` Linux build runs in
hosted CI (needs WebKitGTK).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/tauri_tray.rs src-tauri/src/lib.rs src-tauri/src/events_sink.rs feature_list.json claude-progress.md
git commit -m "feat: Tauri tray on macOS and Linux"
```

Extend the `vol-076` entry with the TauriTray wiring evidence in the same
change set.

---

### Task 3: Phase A docs (README + checklist)

**Files:**
- Modify: `README.md`, `README.vi.md`
- Modify: `docs/testing/cross-platform-release-checklist.md`

- [ ] **Step 1: Update the platform table**

`README.md`/`README.vi.md` System tray row: macOS → `✅ Tauri tray
(menu-bar — manual)`, Linux → `✅ Tauri tray (appindicator — manual)`. Keep
the checklist's Strong/Partial labels honest: creation is CI-verified;
menu interaction is manual.

- [ ] **Step 2: Extend the checklist**

In `docs/testing/cross-platform-release-checklist.md`, add a macOS
"menu-bar tray" manual row and a Linux "StatusNotifier/appindicator tray"
manual row with the evidence fields required (OS version, desktop session,
click-through behavior, screenshots).

- [ ] **Step 3: Commit**

```bash
git add README.md README.vi.md docs/testing/cross-platform-release-checklist.md
git commit -m "docs: platform table and checklist for Linux/macOS tray"
```

---

## Phase B — Overlay HUD on macOS and Linux

### Task 4: WindowManager overlay surface

**Files:**
- Modify: `src-tauri/src/window_manager.rs`
- Modify: `src-tauri/src/lib.rs` (parser test)

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `SurfaceId::Overlay` — label `"window-overlay"`, title `"VolumeControl
    Overlay"`, entry `"src/overlay/index.html"`.
  - `SurfaceId::all()` now returns 4 surfaces (overlay last).
  - `place_surface(SurfaceId::Overlay, work_area, scale)` → bottom-right at
    `OVERLAY_MARGIN_X/Y` (20/40 physical px), 336×88 logical.
  - Overlay windows are created `visible(false)`, non-focusable, and
    `surface_ready` does not focus them.

- [ ] **Step 1: Write the failing geometry/parser tests**

Add to `window_manager.rs` tests:

```rust
#[test]
fn overlay_places_bottom_right_at_legacy_margins() {
    let rect = place_surface(SurfaceId::Overlay, wa(0, 0, 2560, 1400), 1.0);
    assert_eq!(rect.size, PhysicalSize::new(336, 88));
    assert_eq!(rect.position, PhysicalPosition::new(2560 - 20 - 336, 1400 - 40 - 88));
}

#[test]
fn overlay_scales_with_dpi() {
    let rect = place_surface(SurfaceId::Overlay, wa(0, 0, 3840, 2100), 1.5);
    assert_eq!(rect.size, PhysicalSize::new(504, 132));
    assert_eq!(rect.position, PhysicalPosition::new(3840 - 20 - 504, 2100 - 40 - 132));
}

#[test]
fn overlay_handles_negative_origin_work_area() {
    let work = wa(-1920, 0, 1920, 1080);
    let rect = place_surface(SurfaceId::Overlay, work, 1.0);
    assert_eq!(rect.position, PhysicalPosition::new(-20 - 336, 1080 - 40 - 88));
}
```

Update the existing `surface_labels_and_entries_are_stable` and
`all_covers_every_surface` tests for the new surface, and update
`verify_surface_parser_accepts_only_webview_labels` in `lib.rs`:

```rust
assert_eq!(
    super::parse_verify_surface("window-overlay").unwrap(),
    Some(SurfaceId::Overlay)
);
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p volumecontrol-tauri --no-default-features`

Expected: FAIL — `SurfaceId::Overlay` does not exist.

- [ ] **Step 3: Implement the surface**

In `window_manager.rs`:

```rust
pub const OVERLAY_SIZE: (f64, f64) = (336.0, 88.0);
```

Add `Overlay` to the enum, `label()`, `title()`, `entry()`, `all()`, and
`from_label()`. In `place_surface`:

```rust
SurfaceId::Overlay => {
    let w = OVERLAY_SIZE.0 * scale;
    let h = OVERLAY_SIZE.1 * scale;
    let left = wa_x + wa_w - OVERLAY_MARGIN_X - w;
    let top = wa_y + wa_h - OVERLAY_MARGIN_Y - h;
    (left, top, w, h)
}
```

In `open_impl`:

```rust
let (w, h) = match surface {
    SurfaceId::Overlay => OVERLAY_SIZE,
    SurfaceId::Mixer => MIXER_SIZE,
    SurfaceId::Settings => SETTINGS_SIZE,
    SurfaceId::Help => HELP_SIZE,
};
```

Add the overlay builder branch (before `Mixer`):

```rust
SurfaceId::Overlay => {
    builder = builder
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false);
}
```

and after `build()` succeeds, for Overlay:

```rust
if surface == SurfaceId::Overlay {
    let _ = window.set_focusable(false);
    #[cfg(target_os = "macos")]
    {
        // `set_ignore_cursor_events` lives on the underlying tao window; use
        // it via `window.as_ref().window()` if `WebviewWindow` does not
        // forward it directly.
        let _ = window.set_ignore_cursor_events(true);
    }
}
```

In `surface_ready_impl`, only call `set_focus()` for non-Overlay surfaces.
In `open_impl`'s already-open branch, skip `set_focus` for Overlay too
(reposition + show only). Keep the mixer `Focused(false)` auto-close logic
unchanged.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p volumecontrol-tauri --no-default-features`

Expected: all window_manager tests pass, including the four new overlay
geometry tests and the updated parser test.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/window_manager.rs src-tauri/src/lib.rs feature_list.json claude-progress.md
git commit -m "feat: add overlay webview surface"
```

Records in the same change set: `vol-077` entry (status `in_progress`,
geometry/parser test evidence) + the matching `claude-progress.md` session
entry.

---

### Task 5: macOS transparency feature + overlay sink

**Files:**
- Modify: `src-tauri/Cargo.toml` (tauri features += `macos-private-api`)
- Modify: `src-tauri/tauri.conf.json` (app `macOSPrivateApi: true`)
- Modify: `src-tauri/src/events_sink.rs`

**Interfaces:**
- Consumes: `WindowManager::open/close`, `SurfaceId::Overlay`.
- Produces: `TauriSink::overlay()` shows the HUD on macOS/Linux with a
  cancellable auto-hide; `state://overlay` payload:
  `{ text: Option<String>, pct: u8, muted: bool, green_up_to: u8,
  blue_up_to: u8, orange_up_to: u8, theme: String, material: String,
  motion: String, accent: String }`.

- [ ] **Step 1: Write the failing payload-shape test**

Add a test-only helper in `events_sink.rs` that builds the payload from a
`Config` + `VolumeState` (pure, no app handle):

```rust
fn overlay_payload(
    text: Option<String>,
    state: &volumectl_lib::audio::VolumeState,
    config: &volumectl_lib::config::Config,
) -> serde_json::Value {
    serde_json::json!({
        "text": text,
        "pct": state.percent(),
        "muted": state.muted,
        "green_up_to": config.color_thresholds.green_up_to,
        "blue_up_to": config.color_thresholds.blue_up_to,
        "orange_up_to": config.color_thresholds.orange_up_to,
        "theme": format!("{:?}", config.appearance.theme),
        "material": format!("{:?}", config.appearance.material),
        "motion": format!("{:?}", config.appearance.motion),
        "accent": format!("{:?}", config.appearance.accent),
    })
}

#[cfg(test)]
mod tests {
    use super::overlay_payload;

    #[test]
    fn overlay_payload_carries_state_and_thresholds() {
        let state = volumectl_lib::audio::VolumeState { volume: 0.42, muted: true };
        let config = volumectl_lib::config::Config::default();
        let payload = overlay_payload(None, &state, &config);
        assert_eq!(payload["pct"], 42);
        assert_eq!(payload["muted"], true);
        assert_eq!(payload["green_up_to"], config.color_thresholds.green_up_to);
        assert_eq!(payload["text"], serde_json::Value::Null);
    }
}
```

Run: `cargo test -p volumecontrol-tauri --no-default-features overlay_payload`

Expected: FAIL — helper does not exist (or the Config field names differ;
align them with `crates/volumectl/src/config.rs`).

- [ ] **Step 2: Implement the helper and sink path**

Add `overlay_payload` (adjust field access to the real `Config` field names).
Give `TauriSink` a hide-generation counter:

```rust
pub struct TauriSink {
    app: AppHandle,
    overlay_seq: std::sync::Arc<std::sync::Mutex<u64>>,
}
```

In `new`, initialize `overlay_seq`. Replace the non-Windows no-op in
`overlay()` with:

```rust
#[cfg(not(target_os = "windows"))]
{
    use tauri::Emitter;
    let wm = self.app.state::<WindowManager>();
    if let Err(e) = wm.open(SurfaceId::Overlay) {
        log::warn!("overlay open failed: {e}");
        return;
    }
    let _ = self
        .app
        .emit("state://overlay", overlay_payload(text, &state, &config));

    let seq = {
        let mut s = self.overlay_seq.lock().unwrap_or_else(|p| p.into_inner());
        *s += 1;
        *s
    };
    let duration = config.overlay_duration_ms.clamp(200, 10_000);
    let handle = self.app.clone();
    let seq_arc = self.overlay_seq.clone();
    tauri::async_runtime::spawn(async move {
        tauri::async_runtime::sleep(std::time::Duration::from_millis(duration)).await;
        let current = seq_arc.lock().unwrap_or_else(|p| p.into_inner());
        if *current == seq {
            let wm = handle.state::<WindowManager>();
            if let Err(e) = wm.close(SurfaceId::Overlay) {
                log::warn!("overlay auto-hide failed: {e}");
            }
        }
    });
}
```

Keep the Windows branch byte-identical to today.

- [ ] **Step 3: Enable macOS transparency**

`src-tauri/Cargo.toml`:

```toml
tauri = { version = "2", features = ["tray-icon", "image-png", "custom-protocol", "macos-private-api"] }
```

`src-tauri/tauri.conf.json` — under `"app"` add:

```json
"macOSPrivateApi": true
```

If `tauri-build` rejects the key (schema version check), follow its error to
the exact key name and keep the change minimal.

- [ ] **Step 4: Build + test**

Run:

```bash
cargo test -p volumecontrol-tauri --no-default-features
cargo check -p volumecontrol-tauri --no-default-features
```

Expected: payload test passes; Windows build unaffected.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/tauri.conf.json src-tauri/src/events_sink.rs feature_list.json claude-progress.md
git commit -m "feat: cross-platform overlay HUD via EventSink"
```

Extend the `vol-077` entry with the payload/auto-hide evidence in the same
change set.

---

### Task 6: Frontend overlay entry

**Files:**
- Modify: `frontend/vite.config.ts`
- Create: `frontend/src/overlay/index.html`
- Create: `frontend/src/overlay/overlay.tsx`
- Create: `frontend/src/overlay/OverlaySurface.tsx`
- Create: `frontend/src/overlay/OverlaySurface.test.tsx`

**Interfaces:**
- Consumes: `state://overlay` (Task 5 payload), `state://volume`
  (`{ pct, muted }`), `SignalRail` from `../mixer/SignalRail`, `applyAppearance`
  from `../lib/appearance`.
- Produces: the `window-overlay` webview entry rendering the HUD.

- [ ] **Step 1: Write the failing Vitest spec**

`OverlaySurface.test.tsx` (mirror the listen-mocking pattern of
`frontend/src/mixer/MixerSurface.test.tsx`):

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { OverlaySurface } from "./OverlaySurface";

const listeners: Record<string, (payload: unknown) => void> = {};

vi.mock("../lib/ipc", () => ({
  listen: (event: string, cb: (p: unknown) => void) => {
    listeners[event] = cb;
    return Promise.resolve(() => {});
  },
}));

vi.mock("../lib/surface", () => ({
  markSurfaceReady: () => Promise.resolve(),
}));

function fire(event: string, payload: unknown) {
  listeners[event]?.(payload);
}

describe("OverlaySurface", () => {
  it("renders the volume rail and percent from state://overlay", async () => {
    render(<OverlaySurface />);
    fire("state://overlay", {
      text: null,
      pct: 42,
      muted: false,
      green_up_to: 40,
      blue_up_to: 75,
      orange_up_to: 100,
      theme: "dark",
      material: "Auto",
      motion: "Full",
      accent: "System",
    });
    expect(await screen.findByText("42%")).toBeInTheDocument();
    expect(screen.getByTestId("signal-rail")).toBeInTheDocument();
  });

  it("renders a text card instead of the rail when text is present", async () => {
    render(<OverlaySurface />);
    fire("state://overlay", {
      text: "Config reloaded",
      pct: 50,
      muted: false,
      green_up_to: 40,
      blue_up_to: 75,
      orange_up_to: 100,
      theme: "dark",
      material: "Auto",
      motion: "Full",
      accent: "System",
    });
    expect(await screen.findByText("Config reloaded")).toBeInTheDocument();
  });

  it("renders Muted state from the rail", async () => {
    render(<OverlaySurface />);
    fire("state://overlay", {
      text: null,
      pct: 0,
      muted: true,
      green_up_to: 40,
      blue_up_to: 75,
      orange_up_to: 100,
      theme: "dark",
      material: "Auto",
      motion: "Full",
      accent: "System",
    });
    expect(await screen.findByText("Muted")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm test --prefix frontend -- --run src/overlay/OverlaySurface.test.tsx`

Expected: FAIL — module not found.

- [ ] **Step 3: Implement the surface**

`frontend/src/overlay/index.html` mirrors `frontend/src/help/index.html`
(same theme preload script, title `VolumeControl Overlay`, module
`./overlay.tsx`). `overlay.tsx` mirrors `help.tsx` (StrictMode + createRoot +
`OverlaySurface` + `../styles.css`).

`OverlaySurface.tsx`:

```tsx
import { useEffect, useState } from "react";
import { applyAppearance } from "../lib/appearance";
import { listen } from "../lib/ipc";
import { markSurfaceReady } from "../lib/surface";
import {
  DEFAULT_THRESHOLDS,
  SignalRail,
  type ColorThresholds,
} from "../mixer/SignalRail";

interface OverlayPayload {
  text: string | null;
  pct: number;
  muted: boolean;
  green_up_to: number;
  blue_up_to: number;
  orange_up_to: number;
  theme: string;
  material: string;
  motion: string;
  accent: string;
}

export function OverlaySurface() {
  const [payload, setPayload] = useState<OverlayPayload | null>(null);

  useEffect(() => {
    // The host creates the HUD hidden (`visible(false)`); the window only
    // becomes visible after `surface_ready` (same contract as the mixer).
    void markSurfaceReady();
    const unlisteners: Array<() => void> = [];
    void listen<OverlayPayload>("state://overlay", (p) => {
      setPayload(p);
      const resolved =
        p.theme === "Dark"
          ? "dark"
          : p.theme === "Light"
            ? "light"
            : window.matchMedia("(prefers-color-scheme: dark)").matches
              ? "dark"
              : "light";
      applyAppearance({
        theme_resolved: resolved,
        material: p.material,
        motion: p.motion,
        accent: p.accent,
      });
    }).then((unlisten) => unlisteners.push(unlisten));
    return () => unlisteners.forEach((un) => un());
  }, []);

  if (!payload) return null;

  const thresholds: ColorThresholds = {
    green_up_to: payload.green_up_to,
    blue_up_to: payload.blue_up_to,
    orange_up_to: payload.orange_up_to,
  };

  return (
    <div
      data-surface="overlay"
      className="pointer-events-none flex h-screen w-screen items-center justify-center bg-transparent"
    >
      <div
        data-testid="overlay-card"
        className="flex w-[336px] flex-col gap-2 rounded-xl border border-border/60 bg-background/88 px-4 py-3 shadow-lg backdrop-blur-md"
      >
        {payload.text ? (
          <p className="text-center text-sm font-medium text-foreground">
            {payload.text}
          </p>
        ) : (
          <>
            <div className="flex items-baseline justify-between">
              <span className="text-xs uppercase tracking-wide text-foreground/70">
                Volume
              </span>
              <span className="text-2xl font-semibold tabular-nums text-foreground">
                {payload.pct}%
              </span>
            </div>
            <SignalRail
              value={payload.pct}
              muted={payload.muted}
              thresholds={thresholds}
            />
          </>
        )}
      </div>
    </div>
  );
}
```

Check `frontend/src/lib/ipc.ts` for the real `listen` signature and
`applyAppearance`'s argument type; align imports. If `SignalRail` is not
exported from `../mixer/SignalRail` exactly as written, export it.

- [ ] **Step 4: Register the Vite entry**

`frontend/vite.config.ts` `rollupOptions.input`:

```ts
overlay: resolve(import.meta.dirname, "src/overlay/index.html"),
```

- [ ] **Step 5: Run tests + build**

Run:

```bash
npm test --prefix frontend -- --run src/overlay/OverlaySurface.test.tsx
npm test --prefix frontend
npm run build --prefix frontend
```

Expected: new tests pass, full frontend suite green, build emits
`frontend/dist/src/overlay/index.html`.

- [ ] **Step 6: Commit**

```bash
git add frontend/vite.config.ts frontend/src/overlay feature_list.json claude-progress.md
git commit -m "feat: overlay HUD webview surface"
```

Extend the `vol-077` entry with the Vitest/build evidence in the same change
set.

---

### Task 7: Overlay E2E spec and wrapper wiring

**Files:**
- Create: `e2e/tauri/specs/overlay.e2e.ts`
- Modify: `scripts/verify-tauri-e2e.ps1`, `scripts/verify-tauri-e2e.sh`
- Modify: `e2e/tauri/run-debug-e2e.mjs` (surface map + startup surface +
  all list)
- Modify: `e2e/tauri/support/selectors.ts` (SurfaceName + overlay selectors)

- [ ] **Step 1: Write the failing spec**

Mirror the structure of `e2e/tauri/specs/help.e2e.ts` (bootstrap + surface
assertions) with these assertions for the overlay:

```ts
describe("overlay surface", () => {
  it("opens via the debug marker, renders the HUD, and auto-hides", async () => {
    const browser = await getBrowser();
    await browser.tauri?.switchWindow?.("window-overlay");
    const root = await browser.$('[data-surface="overlay"]');
    await root.waitForDisplayed({ timeout: 10_000 });
    const card = await browser.$('[data-surface="overlay"] [data-testid="overlay-card"]');
    await card.waitForDisplayed({ timeout: 10_000 });
    // Auto-hide: the default overlay duration (config default 1800 ms) must
    // destroy the window; the debug run needs no interaction.
    await root.waitForExist({ timeout: 12_000, reverse: true });
  });
});
```

Adjust to the exact helper API of `e2e/tauri/support` (read `help.e2e.ts`
first and reuse its browser/`switchWindow` helpers verbatim; the overlay is
the startup surface for this spec, so `openSurface` is not called).

- [ ] **Step 2: Wire the wrappers**

`scripts/verify-tauri-e2e.ps1`: `[ValidateSet(...)]` gains `overlay`;
the `all` expected-spec array gains `"overlay.e2e.ts"`. Same in
`scripts/verify-tauri-e2e.sh` (`all` list and the single-surface case).
`e2e/tauri/run-debug-e2e.mjs`: add `overlay: ["overlay.e2e.ts"]` to
`specsBySurface`, include `"overlay"` in the `all` surfaces list, and extend
the startup-surface rule to `["settings", "help", "overlay"]` so
`VOLUMECTL_VERIFY_SURFACE=window-overlay` starts the HUD.
`e2e/tauri/support/selectors.ts`: `SurfaceName` gains `"overlay"`;
`surfaceTitle("overlay")` returns
`'[data-surface="overlay"] [data-testid="overlay-card"]'` (the overlay has
no `h1`).

- [ ] **Step 3: Run the E2E contract locally (Windows)**

Run:

```bash
npm run test:e2e:debug --prefix e2e/tauri -- --surface overlay
```

Expected: the overlay opens via `VOLUMECTL_VERIFY_SURFACE=window-overlay`,
renders, and auto-hides (the debug wrapper sets the verify marker; confirm in
`e2e/tauri/support` how markers are passed and reuse that).

- [ ] **Step 4: Run the wrapper evidence gate**

Run:

```bash
powershell -ExecutionPolicy Bypass -File scripts/verify-tauri-e2e.ps1 -Surface overlay -OutputRoot output/tauri-e2e/overlay-local
```

Expected: JUnit + manifest + timings evidence written and asserted (exit 0).

- [ ] **Step 5: Commit**

```bash
git add e2e/tauri/specs/overlay.e2e.ts e2e/tauri/run-debug-e2e.mjs e2e/tauri/support/selectors.ts scripts/verify-tauri-e2e.ps1 scripts/verify-tauri-e2e.sh feature_list.json claude-progress.md
git commit -m "test: overlay surface E2E coverage"
```

Extend the `vol-077` entry with the E2E evidence in the same change set.

---

### Task 8: Phase B docs (README)

**Files:**
- Modify: `README.md`, `README.vi.md`

- [ ] **Step 1: Update the platform table**

Overlay rows: macOS → `✅ webview HUD (click-through — manual)`, Linux →
`✅ webview HUD (auto-hide; click-through n/a)`. Keep the renderer row
unchanged.

- [ ] **Step 2: Commit**

```bash
git add README.md README.vi.md
git commit -m "docs: platform table for Linux/macOS overlay"
```

---

## Phase C — Linux per-app audio (PulseAudio)

### Task 9: PulseSessions backend

**Files:**
- Create: `crates/volumectl/src/audio_sessions_linux.rs`
- Modify: `crates/volumectl/src/lib.rs` (gate the module to Linux)
- Modify: `crates/volumectl/Cargo.toml` (add `libc` for `timeval`)

**Interfaces:**
- Consumes: `crate::host_core::{AudioSessionInfo, SessionsSource}`.
- Produces:
  - `pub(crate) struct PulseSessions` with
    `impl PulseSessions { pub fn new() -> Self }`
  - `impl SessionsSource for PulseSessions` (`supported() -> true`,
    `list() -> Vec<AudioSessionInfo>`, `set_volume(id, pct)`,
    `mute(id)`)
  - `fn build_session_info(index: u32, app_name: Option<&str>,
    media_name: Option<&str>, volume_avg: u32, muted: bool, corked: bool)
    -> AudioSessionInfo` — pure, unit-testable, id = index string, pct =
    `volume_avg * 100 / 65_536` clamped to 100, `active = !corked`.

- [ ] **Step 1: Write the failing pure-mapping tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_prefers_application_name_then_media_name_then_fallback() {
        let app = build_session_info(7, Some("Spotify"), Some("track.mp3"), 32_768, false, false);
        assert_eq!(app.id, "7");
        assert_eq!(app.name, "Spotify");
        assert_eq!(app.pct, 50);
        assert!(!app.muted);
        assert!(app.active);

        let media = build_session_info(8, None, Some("Firefox"), 65_536, true, false);
        assert_eq!(media.name, "Firefox");
        assert_eq!(media.pct, 100);
        assert!(media.muted);

        let fallback = build_session_info(9, None, None, 0, false, true);
        assert_eq!(fallback.name, "Unknown app");
        assert_eq!(fallback.pct, 0);
        assert!(!fallback.active);
    }

    #[test]
    fn mapping_clamps_volume_to_100() {
        let loud = build_session_info(1, Some("A"), None, 200_000, false, false);
        assert_eq!(loud.pct, 100);
    }

    #[test]
    fn sessions_report_supported() {
        assert!(PulseSessions::new().supported());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p volumectl --lib audio_sessions_linux --no-default-features`

Expected: FAIL — module not found (add `#[cfg(target_os = "linux")] pub(crate)
mod audio_sessions_linux;` to `lib.rs` first, then the failure is the missing
functions).

- [ ] **Step 3: Add the libc dependency**

`crates/volumectl/Cargo.toml`, inside
`[target.'cfg(target_os = "linux")'.dependencies]`:

```toml
libc = "0.2"
```

(`pa_threaded_mainloop_wait_until` takes a `*const libc::timeval`.)

- [ ] **Step 3: Implement the mapping + connection**

```rust
//! Linux per-app audio sessions over PulseAudio sink-inputs.

use std::os::raw::c_void;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use libpulse_sys as pa;

use crate::host_core::{AudioSessionInfo, SessionsSource};

const PA_VOLUME_NORM: u32 = 0x10000;

pub fn build_session_info(
    index: u32,
    app_name: Option<&str>,
    media_name: Option<&str>,
    volume_avg: u32,
    muted: bool,
    corked: bool,
) -> AudioSessionInfo {
    let name = app_name
        .filter(|s| !s.is_empty())
        .or_else(|| media_name.filter(|s| !s.is_empty()))
        .unwrap_or("Unknown app")
        .to_string();
    AudioSessionInfo {
        id: index.to_string(),
        name,
        pct: ((volume_avg.saturating_mul(100)) / PA_VOLUME_NORM).min(100) as u8,
        muted,
        active: !corked,
    }
}

struct Connection {
    mainloop: *mut pa::pa_threaded_mainloop,
    context: *mut pa::pa_context,
}

// SAFETY: all Pulse access is serialized behind `PulseSessions.inner` (the
// AppCore mutex is the single caller), matching the LinuxAudio precedent.
unsafe impl Send for Connection {}
unsafe impl Sync for Connection {}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe {
            if !self.context.is_null() {
                pa::pa_context_disconnect(self.context);
                pa::pa_context_unref(self.context);
            }
            if !self.mainloop.is_null() {
                pa::pa_threaded_mainloop_stop(self.mainloop);
                pa::pa_threaded_mainloop_free(self.mainloop);
            }
        }
    }
}

pub struct PulseSessions {
    inner: Mutex<Option<Connection>>,
}

impl PulseSessions {
    pub fn new() -> Self {
        Self { inner: Mutex::new(None) }
    }

    fn connect(&self) -> Option<&Connection> {
        // Ownership dance: connect under the lock, return a raw pointer-free
        // guard by keeping the lock held for the caller's operation. To keep
        // the borrow simple, list/set/mute lock the mutex themselves and call
        // `with_connection` helpers below.
        self.ensure_connected()
    }

    fn ensure_connected(&self) -> std::sync::MutexGuard<'_, Option<Connection>> {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if inner.is_none() {
            *inner = open_connection();
        }
        inner
    }
}

fn open_connection() -> Option<Connection> {
    unsafe {
        let mainloop = pa::pa_threaded_mainloop_new();
        if mainloop.is_null() {
            return None;
        }
        if pa::pa_threaded_mainloop_start(mainloop) < 0 {
            pa::pa_threaded_mainloop_free(mainloop);
            return None;
        }
        let api = pa::pa_threaded_mainloop_get_api(mainloop);
        let context = pa::pa_context_new(api, c"VolumeControl".as_ptr());
        if context.is_null() {
            pa::pa_threaded_mainloop_stop(mainloop);
            pa::pa_threaded_mainloop_free(mainloop);
            return None;
        }

        let conn = Connection { mainloop, context };
        let state_ptr = &conn as *const Connection as *mut c_void;
        pa::pa_context_set_state_callback(
            context,
            Some(state_cb),
            state_ptr,
        );
        pa::pa_context_connect(context, std::ptr::null(), pa::PA_CONTEXT_NOFLAGS, std::ptr::null());

        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let state = pa::pa_context_get_state(context);
            if state == pa::pa_context_state_t::PA_CONTEXT_READY {
                return Some(conn);
            }
            if state == pa::pa_context_state_t::PA_CONTEXT_FAILED
                || state == pa::pa_context_state_t::PA_CONTEXT_TERMINATED
            {
                return None; // conn dropped by caller path below
            }
            if Instant::now() >= deadline {
                return None;
            }
            wait_mainloop(mainloop, deadline);
        }
    }
}

unsafe extern "C" fn state_cb(
    _context: *mut pa::pa_context,
    userdata: *mut c_void,
) {
    let conn = &*(userdata as *const Connection);
    pa::pa_threaded_mainloop_signal(conn.mainloop, 0);
}

fn wait_mainloop(mainloop: *mut pa::pa_threaded_mainloop, deadline: Instant) {
    let remaining = deadline.saturating_duration_since(Instant::now());
    let tv = libc::timeval {
        tv_sec: remaining.as_secs() as _,
        tv_usec: remaining.subsec_micros() as _,
    };
    unsafe { pa::pa_threaded_mainloop_wait_until(mainloop, &tv) };
}

impl SessionsSource for PulseSessions {
    fn supported(&self) -> bool {
        true
    }

    fn list(&self) -> Vec<AudioSessionInfo> {
        let mut inner = self.ensure_connected();
        let Some(conn) = inner.as_mut() else {
            return Vec::new();
        };
        let mut sessions: Vec<AudioSessionInfo> = Vec::new();
        let userdata = &mut sessions as *mut Vec<AudioSessionInfo> as *mut c_void;
        let op = unsafe {
            pa::pa_context_get_sink_input_info_list(
                conn.context,
                Some(sink_input_info_cb),
                userdata,
            )
        };
        let ok = run_op(conn, op, |mainloop| unsafe {
            pa::pa_threaded_mainloop_signal(mainloop, 0)
        });
        if !ok {
            log::warn!("pulse: sink-input enumeration failed");
            sessions.clear();
        }
        sessions
    }

    fn set_volume(&self, id: &str, pct: u8) -> Result<(), String> {
        let mut inner = self.ensure_connected();
        let Some(conn) = inner.as_mut() else {
            return Err("pulse unavailable".to_string());
        };
        let index: u32 = id
            .parse()
            .map_err(|_| format!("session {id:?} is not a sink-input index"))?;
        let mut cvol = pa::pa_cvolume {
            channels: 1,
            values: [0; pa::PA_CHANNELS_MAX],
        };
        let norm = (pct as u32 * PA_VOLUME_NORM).min(PA_VOLUME_NORM * 100) / 100;
        unsafe {
            pa::pa_cvolume_set(&mut cvol, 1, norm);
            let op = pa::pa_context_set_sink_input_volume(
                conn.context,
                index,
                &cvol,
                Some(success_cb),
                std::ptr::null_mut(),
            );
            if !run_op(conn, op, |mainloop| unsafe {
                pa::pa_threaded_mainloop_signal(mainloop, 0)
            }) {
                return Err(format!("session {id:?} no longer active"));
            }
        }
        Ok(())
    }

    fn mute(&self, id: &str) -> Result<(), String> {
        let mut inner = self.ensure_connected();
        let Some(conn) = inner.as_mut() else {
            return Err("pulse unavailable".to_string());
        };
        let index: u32 = id
            .parse()
            .map_err(|_| format!("session {id:?} is not a sink-input index"))?;
        // Read the current mute under the same connection, then flip it.
        let mut muted = false;
        let userdata = &mut muted as *mut bool as *mut c_void;
        let op = unsafe {
            pa::pa_context_get_sink_input_info(
                conn.context,
                index,
                Some(sink_input_mute_cb),
                userdata,
            )
        };
        if !run_op(conn, op, |mainloop| unsafe {
            pa::pa_threaded_mainloop_signal(mainloop, 0)
        }) {
            return Err(format!("session {id:?} no longer active"));
        }
        let op = unsafe {
            pa::pa_context_set_sink_input_mute(
                conn.context,
                index,
                if muted { 0 } else { 1 },
                Some(success_cb),
                std::ptr::null_mut(),
            )
        };
        if !run_op(conn, op, |mainloop| unsafe {
            pa::pa_threaded_mainloop_signal(mainloop, 0)
        }) {
            return Err(format!("session {id:?} no longer active"));
        }
        Ok(())
    }
}

unsafe extern "C" fn sink_input_info_cb(
    _c: *mut pa::pa_context,
    info: *const pa::pa_sink_input_info,
    eol: libc::c_int,
    userdata: *mut c_void,
) {
    if eol != 0 || info.is_null() {
        return;
    }
    let sessions = &mut *(userdata as *mut Vec<AudioSessionInfo>);
    let info = &*info;
    let app_name = proplist_get(info.proplist, pa::PA_PROP_APPLICATION_NAME);
    let media_name = proplist_get(info.proplist, pa::PA_PROP_MEDIA_NAME);
    let avg = unsafe { pa::pa_cvolume_avg(&info.volume) };
    sessions.push(build_session_info(
        info.index,
        app_name.as_deref(),
        media_name.as_deref(),
        avg,
        info.mute != 0,
        info.corked != 0,
    ));
}

unsafe extern "C" fn sink_input_mute_cb(
    _c: *mut pa::pa_context,
    info: *const pa::pa_sink_input_info,
    eol: libc::c_int,
    userdata: *mut c_void,
) {
    if eol == 0 && !info.is_null() {
        let muted = &mut *(userdata as *mut bool);
        *muted = (*info).mute != 0;
    }
}

unsafe extern "C" fn success_cb(
    _c: *mut pa::pa_context,
    success: libc::c_int,
    _userdata: *mut c_void,
) {
    let _ = success;
}

fn proplist_get(plist: *mut pa::pa_proplist, key: &[u8]) -> Option<String> {
    if plist.is_null() {
        return None;
    }
    let ptr = unsafe { pa::pa_proplist_gets(plist, key.as_ptr() as *const libc::c_char) };
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().into_owned())
}

fn run_op(
    conn: &Connection,
    op: *mut pa::pa_operation,
    signal: impl Fn(*mut pa::pa_threaded_mainloop),
) -> bool {
    if op.is_null() {
        return false;
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let state = unsafe { pa::pa_operation_get_state(op) };
        match state {
            pa::pa_operation_state_t::PA_OPERATION_DONE => {
                unsafe { pa::pa_operation_unref(op) };
                return true;
            }
            pa::pa_operation_state_t::PA_OPERATION_CANCELLED
            | pa::pa_operation_state_t::PA_OPERATION_FAILED => {
                unsafe { pa::pa_operation_unref(op) };
                return false;
            }
            _ => {
                if Instant::now() >= deadline {
                    unsafe { pa::pa_operation_cancel(op) };
                    unsafe { pa::pa_operation_unref(op) };
                    return false;
                }
                signal(conn.mainloop);
                wait_mainloop(conn.mainloop, deadline);
            }
        }
    }
}
```

Notes for the implementer (not placeholders — real FFI details to confirm
against `libpulse-sys` at compile time): if `pa_threaded_mainloop_wait_until`
is not bound, replace `wait_mainloop` with a bounded loop of
`pa_threaded_mainloop_wait` + `Instant` checks (the callbacks always signal,
so the loop terminates). If `c"..."` literals need Rust 1.77+, use
`b"VolumeControl\0".as_ptr() as *const libc::c_char`. Confirm enum constant
names (`PA_CONTEXT_READY`, `PA_OPERATION_DONE`) against the crate; libpulse-sys
exposes them as `pa_context_state_t::PA_CONTEXT_READY` style or plain `u32`
constants — use whichever the crate exports. `PA_PROP_APPLICATION_NAME` and
`PA_PROP_MEDIA_NAME` are byte-string constants in bindgen output — they are
passed to `proplist_get(plist, pa::PA_PROP_APPLICATION_NAME)` directly thanks
to the `&[u8]` key parameter. If `PA_CHANNELS_MAX` is not exported, use the
literal `64` (the PulseAudio channel hard cap).

- [ ] **Step 4: Run the mapping tests**

Run: `cargo test -p volumectl --lib audio_sessions_linux --no-default-features`

Expected: PASS (mapping tests are pure; the connection code compiles without
libpulse at link time on Linux CI where `libpulse-dev` is installed).

- [ ] **Step 5: No-server degradation test**

Add (Linux-only; it must not depend on a running Pulse server):

```rust
#[test]
fn no_pulse_server_degrades_to_empty_list() {
    // Point Pulse at a dead socket so the test is hermetic regardless of the
    // runner's audio state. Restore the previous value afterwards.
    let old = std::env::var_os("PULSE_SERVER");
    std::env::set_var("PULSE_SERVER", "tcp:127.0.0.1:1");
    let sessions = PulseSessions::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let listed = sessions.list();
        assert!(
            std::time::Instant::now() < deadline,
            "list() must return quickly when Pulse is unreachable"
        );
        assert!(listed.is_empty(), "unreachable Pulse must yield no sessions");
    }));
    match old {
        Some(v) => std::env::set_var("PULSE_SERVER", v),
        None => std::env::remove_var("PULSE_SERVER"),
    }
    assert!(result.is_ok(), "no-server degradation test panicked");
}
```

- [ ] **Step 6: Commit**

```bash
git add crates/volumectl/src/audio_sessions_linux.rs crates/volumectl/src/lib.rs crates/volumectl/Cargo.toml Cargo.lock feature_list.json claude-progress.md
git commit -m "feat: Linux PulseAudio per-app sessions"
```

Records in the same change set: `vol-078` entry (status `in_progress`,
mapping + no-server degradation test evidence) + the matching
`claude-progress.md` session entry. `Cargo.lock` changes from the `libc`
addition belong in this commit too.

---

### Task 10: Wire PulseSessions into AppCore + frontend copy

**Files:**
- Modify: `crates/volumectl/src/host_core.rs`
- Modify: `frontend/src/mixer/MixerSurface.tsx`
- Modify: `frontend/src/mixer/MixerSurface.test.tsx`

- [ ] **Step 1: Write the failing wiring test**

In `crates/volumectl/tests/host_core.rs` (integration target; the file
already defines `StubAudio`, `RecordingSink`, and `core_with`), add a
Linux-gated test that the default session source reports supported on Linux:

```rust
#[cfg(target_os = "linux")]
#[test]
fn linux_core_uses_a_supported_sessions_source() {
    let mut core = core_with(Arc::new(RecordingSink::default()));
    // Bootstrap reports per-app sessions as supported on Linux.
    let payload = core.bootstrap();
    assert!(payload.sessions_supported);
}
```

`core_with`, `StubAudio`, and `RecordingSink` already exist in that file —
reuse them verbatim.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p volumectl --test host_core linux_core_uses_a_supported_sessions_source --no-default-features`

Expected: FAIL — `sessions_supported` is false (NoopSessions on Linux).

- [ ] **Step 3: Wire the source**

In `host_core.rs` `new_inner`:

```rust
#[cfg(target_os = "linux")]
let sessions_source: Box<dyn SessionsSource> = Box::new(crate::audio_sessions_linux::PulseSessions::new());
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
let sessions_source: Box<dyn SessionsSource> = Box::new(NoopSessions);
```

(Keep the existing Windows branch unchanged.)

- [ ] **Step 4: Update the frontend notice**

`frontend/src/mixer/MixerSurface.tsx` line ~140: replace
`Per-app mixing is Windows-only` with
`Per-app audio isn't available on this platform.`

Update `MixerSurface.test.tsx`: rename the test to
`"shows the unsupported-platform notice when sessions are unsupported"` and
assert the new string.

- [ ] **Step 5: Run the suites**

Run:

```bash
cargo test -p volumectl --test host_core --no-default-features
cargo test --workspace --no-default-features
npm test --prefix frontend
```

Expected: all green, including the new Linux-gated wiring test.

- [ ] **Step 6: Commit**

```bash
git add crates/volumectl/src/host_core.rs crates/volumectl/tests/host_core.rs frontend/src/mixer/MixerSurface.tsx frontend/src/mixer/MixerSurface.test.tsx feature_list.json claude-progress.md
git commit -m "feat: enable PulseAudio sessions on Linux"
```

Extend the `vol-078` entry with the host-core wiring + frontend copy evidence
in the same change set.

---

### Task 11: Phase C docs (README + checklist)

**Files:**
- Modify: `README.md`, `README.vi.md`
- Modify: `docs/testing/cross-platform-release-checklist.md`

- [ ] **Step 1: Update the platform table**

Mixer rows: Linux → `✅ Tauri + PulseAudio sink-inputs`; macOS →
`✅ Tauri surface / ✖ per-app audio (no public API)`. Update the
`README.vi.md` row identically.

- [ ] **Step 2: Extend the checklist**

Add the Linux manual row: start a Pulse server (`pulseaudio --start`), play
two streams, open the mixer, adjust each sink-input slider, verify the mixer
list updates and survives stream restarts (stale-id path).

- [ ] **Step 3: Commit**

```bash
git add README.md README.vi.md docs/testing/cross-platform-release-checklist.md
git commit -m "docs: platform table and checklist for Linux per-app audio"
```

---

### Task 12: Full verification battery

**Files:**
- Verify all files changed by Tasks 1–11
- Update `feature_list.json`, `claude-progress.md`, `session-handoff.md`

- [ ] **Step 1: Run the local quality gate**

```bash
cargo fmt --all --check
git diff --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
npm test --prefix frontend
npm run build --prefix frontend
bash scripts/test-check-records.sh
bash scripts/test-format-lint.sh
bash scripts/test-ship.sh
sh scripts/check-records.sh --branch origin/main
```

Expected: all exit 0.

- [ ] **Step 2: Cross-target checks**

Run: `cargo check -p volumectl --target x86_64-unknown-linux-gnu --no-default-features`

Expected: clean (validates the Linux-gated session module and un-gated tray
mapping compile for the Linux target from this Windows host; full Tauri
Linux/macOS builds run in hosted CI).

- [ ] **Step 3: Run the pre-push review**

Run the mandatory three-domain `pre-push-review` skill (guard core, gate
chain, wiring/records) on the branch; fix only verified genuine findings with
live negative verification; record the review evidence.

- [ ] **Step 4: Hosted CI + final records**

Push the branch and open a PR (or land through the ship flow on main per
repo policy). After the hosted Ubuntu/macOS/Windows runs complete, record the
run IDs and artifact evidence in `feature_list.json`/`claude-progress.md`.
Only mark vol-076/077/078 `passing` with hosted evidence; tray menu
interaction, macOS overlay transparency/click-through, and real Pulse
sink-input round-trips stay manual rows in the checklist.

- [ ] **Step 5: Update session handoff**

Refresh `session-handoff.md` with the completed phases, evidence paths, and
the remaining manual rows.

---

## Self-review against the spec

- Phase A tray: Tasks 1–3 (shared `TrayCommand`, `TauriTray`, non-fatal
  creation, host lifecycle, docs; records ride in the Task 1/2 commits).
- Phase B overlay: Tasks 4–8 (surface + geometry, macOS private API +
  `EventSink` auto-hide, frontend HUD, E2E, docs; records ride in the
  Task 4–7 commits).
- Phase C per-app audio: Tasks 9–11 (Pulse sink-inputs, host-core wiring,
  truthful macOS boundary + frontend copy, docs; records ride in the
  Task 9/10 commits).
- Boundaries: Windows untouched (all edits cfg-gated or additive); macOS
  per-app audio explicitly not implemented; checklist manual rows cover tray
  hosts, overlay click-through, and real Pulse round-trips.
- Gate: Task 12 runs the full battery plus the mandatory pre-push review.
