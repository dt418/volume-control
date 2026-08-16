# Complete Missing Linux/macOS Features — Design Spec

**Date:** 2026-08-16

**Goal:** Close the three remaining 🔜 rows of the platform status table for
the released Tauri app on macOS and Linux: **system tray**, **overlay HUD**
host integration, and **per-app audio** in the Mixer. Windows behavior stays
unchanged (it is the fully verified baseline).

**Product context:** The released app (v0.1.2) is the Tauri host
(`src-tauri`). Windows already has all three features: a native Win32
overlay/tray (`native_win32.rs`) and WASAPI per-app sessions
(`audio_sessions_win32.rs`). On macOS/Linux the `EventSink` implementations
for overlay and tray-menu are deliberate no-ops, and `AppCore` uses
`NoopSessions`.

## Current-state evidence (authoritative)

- `src-tauri/src/events_sink.rs`: `overlay()` → "no native overlay on
  Linux/macOS"; `show_tray_menu()` → "tray menu unavailable on this platform".
- `src-tauri/src/lib.rs`: `install_menu_event_handler` is `#[cfg(windows)]`;
  no `TrayIconBuilder` exists on any platform (Windows tray is created inside
  `native_win32.rs` via `tray-icon`/`muda`).
- `crates/volumectl/src/host_core.rs`: `SessionsSource` is `WindowsSessions`
  on Windows, `NoopSessions` elsewhere; `system_is_dark()` returns `None` off
  Windows; `ShowSurface(Overlay)` logs "overlay surface not wired".
- `crates/volumectl/src/tray.rs`: `TrayCommand::from_menu_id` maps stable ids
  (`mute`, `reset`, `mixer`, `settings`, `help`, `reload`, `edit`, `exit`);
  `tray_command_to_action` in `host_core.rs` is Windows-gated.
- `crates/volumectl/src/overlay.rs`: legacy overlay is **336×88 logical**,
  bottom-right, always-on-top, click-through; `window_manager.rs` already
  reserves the 88px overlay stack for mixer placement (`OVERLAY_H`,
  `OVERLAY_MARGIN_X/Y`).
- `tauri = { version = "2", features = ["tray-icon", "image-png",
  "custom-protocol"] }` — the Tauri tray feature is already enabled.
- `libpulse-sys = "1.23.0"` is already a `volumectl` dependency (used by
  `audio_linux.rs`); Linux per-app mixing is therefore buildable without new
  native dependencies. CI already installs `libayatana-appindicator3-dev` and
  the GTK/WebKit dependency set.

## Scope and boundaries

**In scope:**

1. Tray icon + the existing tray menu on macOS and Linux (Tauri-built tray).
2. Overlay HUD on macOS and Linux (webview HUD surface driven by
   `EventSink::overlay`).
3. Per-app audio sessions on **Linux only** (PulseAudio sink-inputs).
4. Frontend copy + README/README.vi table updates so every row states the
   truth (macOS per-app audio = **not supported**, not "soon").

**Out of scope (documented, not implemented):**

- macOS per-app audio: there is **no public API** for per-app output volume
  since macOS 10.14; private/hacky approaches (private frameworks,
  AppleScript System Events) are rejected for a distributable release. The
  Mixer keeps its per-app section hidden on macOS with truthful copy.
- Real Wayland compositor, tray-host (StatusNotifierWatcher), hardware
  PulseAudio, and multi-monitor evidence: hosted CI cannot prove these; the
  cross-platform checklist remains the evidence boundary.
- Windows changes: the native overlay/tray stay exactly as-is (no regression
  risk on the fully verified path).

## Recommended order

**Phase A (tray) → Phase B (overlay) → Phase C (Linux per-app audio).**
Each phase is independently reviewable and lands with its own records update.

---

## Phase A — System tray on macOS and Linux

### Approach

Create one Tauri-managed tray (`tauri::tray::TrayIconBuilder`) on macOS and
Linux in `src-tauri`, reusing the exact menu ids/labels of the Windows native
tray so the shared `TrayCommand` mapping stays the single source of truth.
Windows keeps its native tray unchanged.

Alternative considered and rejected: replacing the Windows native tray with
the Tauri tray on all platforms. It would unify the code but re-verifies a
fully working Windows surface and changes the overflow-flyout/dismiss
behavior — rejected for regression risk.

### Components

- **`TauriTray`** (new `src-tauri/src/tauri_tray.rs`):
  - `create(app) -> Result<TauriTray, String>` builds the menu (live volume
    label, Mute check, Reset to 50%, Open mixer, Settings, Help, Reload
    configuration, Open config file, Exit VolumeControl) with the stable
    ids, plus a tooltip and the generated speaker icon.
  - `set_volume(&VolumeState)` updates the volume label + mute check +
    tooltip (mirrors `native_win32::set_tray_volume`).
  - `show_menu()` where the platform/tray backend supports popping the menu
    (OpenMenu hotkey); otherwise logs. Linux appindicator menus open on
    click; this is documented as a platform boundary.
  - Creation failure is **non-fatal**: log + keep the host alive (degraded
    capability, same pattern as `UnavailableAudio`).
- **Icon:** move the generated speaker RGBA helper (`tray_icon_rgba` in
  `crates/volumectl/src/tray.rs`) to a shared pub fn so both the native
  Windows tray and the Tauri tray use it (`tauri::image::Image::new_owned`).
- **Menu events:** one handler per platform, never two. Windows keeps the
  existing global `Builder::on_menu_event` (delegating to a shared
  `dispatch_tray_command`); macOS/Linux register the Tauri tray's own
  `TrayIconBuilder::on_menu_event` callback, which calls the same
  `dispatch_tray_command(TrayCommand::from_menu_id(...))` helper. This
  avoids any double-dispatch risk between the global and tray-specific
  handlers.
- **Shared mapping:** remove the `#[cfg(target_os = "windows")]` gates from
  `TrayCommand::from_menu_id` and `host_core::tray_command_to_action`
  (both are pure mappings; unit tests already cover them).
- **Host lifecycle:** `EXIT_REQUESTED` + `RunEvent::ExitRequested` handling
  already keeps the tray host alive on every platform — macOS/Linux gain the
  resident host + tray Exit path for free once the tray exists.
- **Sink:** `TauriSink::show_tray_menu()` on macOS/Linux calls
  `TauriTray::show_menu()`; `TauriSink::volume()` also refreshes the Tauri
  tray label (guarded by `try_state`, like Windows).

### Verification (Phase A)

- Unit: `TrayCommand::from_menu_id` + `tray_command_to_action` tests run on
  all platforms (currently Windows-gated).
- CI Ubuntu/Xvfb + macOS: app starts, tray creation is attempted, menu event
  handler is installed, no crash; a `VOLUMECTL_VERIFY_SURFACE`-style debug
  marker can assert tray state was managed. Tray-host presence (appindicator
  watcher on Linux, menu bar on macOS) is **manual/partial** evidence.
- Full format-lint gate + records guard on the change set.

---

## Phase B — Overlay HUD on macOS and Linux

### Approach

Add a webview HUD surface (`window-overlay`) driven by the existing
`EventSink::overlay(text, state, config)` notification. This reuses the
hybrid webview architecture (like Mixer/Settings/Help) and the existing
SignalRail frontend component. A transparent, always-on-top, skip-taskbar,
non-focusable window renders the volume capsule (336×88 logical, bottom-right
at 20/40 physical margins — the legacy geometry `window_manager.rs` already
reserves) or a short text card ("Config reloaded").

### Transparency decision

- **Linux (X11/WebKitGTK):** `transparent(true)` works with an RGBA visual
  under a compositor; Xvfb has no compositor so CI asserts creation/render,
  and real-desktop appearance is manual evidence.
- **macOS:** Tauri transparent windows require the documented
  `macos-private-api` Cargo feature + per-window `transparent(true)`. This
  app is distributed outside the App Store (ad-hoc signed GitHub releases),
  so the private-API tradeoff is accepted; if a future App Store path
  appears, the overlay falls back to an opaque material card (the same
  fallback the mixer uses today) without behavior loss.
- **Click-through:** macOS `set_ignore_cursor_events(true)`; Linux has no
  Tauri input-shape API — mitigation is short auto-hide (configurable
  `overlay_duration_ms`, 200–10 000 ms), `focusable(false)`, and no
  decorations. This is recorded as a partial/manual boundary on Linux.

### Components

- **`SurfaceId::Overlay`** in `window_manager.rs` (`window-overlay`,
  entry `src/overlay/index.html`, 336×88 logical, placement
  `place_surface` extended for Overlay using the existing
  `OVERLAY_H/MARGIN` constants). It stays out of `SurfaceId::all()` used by
  the three user surfaces; overlay open/close is host-driven only.
- **`TauriSink::overlay()`** on macOS/Linux:
  1. `WindowManager::open(Overlay)` (idempotent; repositions on repeat
     shows);
  2. emit `state://overlay` `{ text, pct, muted, thresholds, theme }` so the
     webview renders without a full bootstrap round-trip;
  3. schedule auto-hide after `config.overlay_duration_ms` through a
     cancellable Tauri async task (new show cancels the previous timer).
- **Frontend:** new `frontend/src/overlay/index.html` +
  `OverlaySurface.tsx` reusing `SignalRail` + `applyAppearance`; subscribes
  to `state://overlay` and `state://volume`; body transparent, pointer-events
  none in CSS.
- **AppCore:** no change needed for the notification path (`publish_state`
  already calls `sink.overlay`); `ShowSurface(Overlay)` log stub stays (the
  sink is the only overlay driver).
- **macOS theme:** `system_is_dark()` gains a macOS probe via the existing
  `objc2-app-kit` dependency (`NSApplication.effectiveAppearance`), so the
  overlay/HUD appearance matches the system theme. Linux keeps `None` (the
  webview's CSS `prefers-color-scheme` is the fallback) unless a cheap
  GTK-settings probe is available — not a Phase-B requirement.

### Verification (Phase B)

- Frontend Vitest: `OverlaySurface` renders volume + text-card modes,
  subscribes, respects reduced motion.
- Rust: `place_surface(Overlay)` geometry tests (physical margins, DPI
  scaling, negative-origin work areas); sink overlay scheduling unit test
  with a fake timer seam where feasible.
- CI Ubuntu/Xvfb + macOS: `VOLUMECTL_VERIFY_SURFACE=window-overlay` debug
  marker opens the HUD, WDIO overlay spec asserts the rendered rail + value,
  and auto-hide destroys the window. Real compositor transparency /
  click-through is manual evidence.

---

## Phase C — Per-app audio (Linux PulseAudio)

### Approach

Implement `SessionsSource` for Linux with a direct `libpulse-sys` threaded
mainloop connection (`PulseSessions`), enumerating **sink-inputs** (per-app
playback streams) and mapping them to the existing `AudioSessionInfo`
contract (`id`, `name`, `pct`, `muted`, `active`). macOS stays `NoopSessions`
with truthful frontend copy.

### Components

- **`crates/volumectl/src/audio_sessions_linux.rs`** — `PulseSessions`:
  - Lazily connects with `pa_threaded_mainloop` on first use; reconnects on
    failure; connection/absence of a server is never fatal (empty list +
    log, commands return `Err` so `AppCore` re-emits the fresh list — the
    existing §9.6 stale-id contract).
  - `list()` → `pa_context_get_sink_input_info_list`; each
    `pa_sink_input_info` maps via a **pure, unit-tested helper**
    `sink_input_info_to_session`: name from `application.name` →
    `media.name` → `"Unknown app"`, `pct` from the volume average (linear
    mapping), `muted`, `active = !corked`, `id` = sink-input index.
  - `set_volume(id, pct)` → `pa_context_set_sink_input_volume`;
    `mute(id)` → `pa_context_set_sink_input_mute`. Unknown/stale index →
    `Err("session ... no longer active")`.
- **Wiring:** `host_core.rs` `new_inner` selects
  `#[cfg(target_os = "linux")] PulseSessions`; all other non-Windows targets
  keep `NoopSessions`.
- **Frontend copy:** the mixer's unsupported notice ("Windows-only") becomes
  platform-neutral: **"Per-app audio isn't available on this platform."**
  Visible on macOS (and Linux when Pulse is absent the empty state remains,
  matching Windows behavior when sessions are unavailable).
- **Docs:** README/README.vi mixer rows: Linux → `PulseAudio sink-inputs`;
  macOS → `not supported` (no more 🔜).

### macOS boundary (explicit)

macOS per-app volume/mute has no public API since 10.14. The honest end
state is a **not-supported** row, not a half-feature. This spec deliberately
does not add private-API or accessibility-permission hacks for it.

### Verification (Phase C)

- Unit tests (Linux-gated): name fallback chain, pct calculation, corked →
  inactive, stale-id error text, `PulseSessions::new()` with no Pulse server
  degrades to an empty list without hanging (connect timeout guard).
- Host-core tests: `AppCore` with an injected fake `SessionsSource` already
  covers bootstrap/`sessions()`/stale-id re-emit; extend with a Linux fake
  where the real Pulse path cannot run headless.
- CI Ubuntu: cargo tests run; a live Pulse sink-input round-trip is **manual
  evidence** (hosted runners do not guarantee a Pulse server), recorded in
  the checklist.
- Full format-lint gate + records guard on the change set.

## Cross-cutting

- Every phase updates `feature_list.json` (new feature entries with
  verification + evidence) and `claude-progress.md` in the same change set.
- README.md / README.vi.md platform table rows change only with the phase
  that proves them; hosted CI evidence is recorded only after the run
  completes.
- `docs/testing/cross-platform-release-checklist.md` gains the manual
  macOS menu-bar / overlay click-through / Linux Pulse sink-input rows.
- No changes to `init.sh`, the release workflow contract, or the Windows
  native surfaces.
