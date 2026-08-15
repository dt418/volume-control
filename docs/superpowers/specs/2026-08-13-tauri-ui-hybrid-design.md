# Hybrid Tauri UI Design: Mixer / Settings / Help → Webview (shadcn/ui)

**Date:** 2026-08-13
**Status:** Approved (design review)
**Branch:** `feature/tauri-ui-hybrid`

## 1. Problem statement

`volumectl` renders every surface with hand-written native code —
Windows GDI/D2D, GTK4 (Linux), AppKit (macOS) — via
`crates/volumectl/src/ui/`. The current UI works but is expensive to evolve:
the Mixer, Settings and Help surfaces have growing visual requirements
(per-app controls, forms, theming, animation) that are painful in three
platform-specific renderers.

The goal is a **hybrid architecture**: keep the native HUD overlay (fast,
stable, already tuned) and the native tray/hotkey/audio core exactly as they
are, and re-implement the three *interactive* surfaces — **Mixer, Settings,
Help** — as Tauri v2 webview windows with a **React + TypeScript + Tailwind +
shadcn/ui** frontend.

Success criteria (agreed with the user):

- **A — Design:** shadcn/ui design system; System/Dark/Light theme; glassmorphic
  surfaces (window-level Mica/Acrylic where the platform allows, in-webview
  backdrop blur otherwise) consistent with the native HUD; lucide-react icons;
  framer-motion transitions.
- **B — Usability:** per-app volume sliders with quick mute, search filter and
  active-first auto-sort in the Mixer; a restricted hotkey recorder (modifier
  picker + key cards) and step-size controls in Settings; card-based, filterable
  shortcut list in Help.
- **C — Behavior:** Rust stays the single source of truth; hotkey-driven volume
  changes push to open webviews in < 2 ms; hotkey conflict indicators in
  Settings; webview windows are lazy-created on demand and destroyed on close
  to protect the app's ultra-low idle RAM profile.

Platform parity: **Windows + Linux + macOS** all keep working; CI gains the
Linux webkit2gtk system packages Tauri needs.

## 2. Locked decisions

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| D1 | Migration scope | **Hybrid** — native HUD + tray + hotkeys + audio stay native; only Mixer/Settings/Help become webview | Overlay is the latency-critical surface; interactive surfaces benefit most from a component framework; least risk to the tuned core |
| D2 | Window model | **3 separate Tauri windows, lazy-created on demand, destroyed on close** (Option B) | Matches the existing surface model; preserves lazy-load; second+ open is fast via shared WebView2/WebKit runtime process |
| D3 | Frontend stack | React 19 + TypeScript + Vite + Tailwind CSS v4 + shadcn/ui (Radix) + lucide-react + framer-motion | User-selected; shadcn gives a consistent, accessible design system |
| D4 | Hotkey recorder | **Restricted** — recorder edits only the configurable modifier; fixed keys (↑ ↓ M R V + Shift variants) shown as read-only key cards | YAGNI: no config-schema change, no free-form key validation/conflict machinery; matches how the config actually works today |
| D5 | Packaging | **Single portable binary** via `tauri build --no-bundle` (assets embedded); ship.sh + CI artifact flow kept | No signing/provisioning/installer complexity; WebView2/WebKit runtimes are present on modern OSes |
| D6 | Appearances | Existing appearance config (theme/material/motion/accent) + adaptive capability resolution is preserved and pushed to webviews as tokens | No regression of the current appearance model |
| D7 | Hotkeys | Keep the plain `global-hotkey` crate + `hotkeys_global` module (no re-migration to tauri-plugin-global-shortcut) | Already integrated, tested, hardened |

## 3. Architecture

The Tauri v2 application process becomes the single host process. All native
pieces keep running in-process; webview windows are created on demand.

```
volume-control (Tauri v2 app process — Rust)
├── Native (unchanged logic, re-homed into the Tauri runtime)
│   ├── AppContext  ──→ tauri::State (SSOT: config, volume state, hotkey status)
│   ├── GlobalHotkeys (hotkeys_global + global-hotkey crate)  ──→ setup hook
│   ├── Wheel hook (win32) / audio core / blacklist / beep     ──→ setup hook
│   ├── Overlay HUD (native win32 window — kept 100% as-is)
│   └── Tray (native, unchanged)
├── WindowManager (new Rust state)
│   ├── open_surface(SurfaceId) → lazy-create WebviewWindow
│   └── close → destroy window (Esc / outside-click / X)
└── 3 webview windows (on-demand)
    └── React SPA — separate entry bundles: mixer / settings / help
```

### 3.1 Re-homing the existing host

Today `app.rs` owns a win32 message loop (`host_wndproc`, `SetTimer`) that
dispatches `WM_APP_*` messages between the tray/overlay/mixer/settings/help
surfaces and the shared `AppContext`. In the hybrid design:

- `AppContext` and the action dispatcher (`handle_action`, `apply_hotkey`,
  volume/mute/reset, blacklist gate, beep) move into the Tauri runtime as
  managed state. No behavioral change to the hotkey/wheel/tray paths.
- The native HUD overlay window and the tray keep their win32/GTK/AppKit
  implementations and are created in the Tauri `setup` hook.
- Cross-surface plumbing is replaced by direct Rust calls (native ↔ native)
  and Tauri events (native → webview).
- The Windows host message loop is replaced by the Tauri event loop; the
  hotkey drain timer and config-sync timer become Tauri `async` timers or
  events from the existing channels (the `GlobalHotkeys` channel + wheel hook
  already emit into channels the host can drain in an async task).

### 3.2 WindowManager (lazy lifecycle)

New Rust state with one `WebviewWindowBuilder` recipe per surface:

| Surface | Label | Default size | Style | Lifecycle |
|---------|-------|--------------|-------|-----------|
| Mixer | `window-mixer` | 420×580 | Frameless, transparent (acrylic/backdrop) | Opened by hotkey/tray; Esc or outside-click → destroy |
| Settings | `window-settings` | 680×520 | Standard header, resizable | Opened from tray; X → destroy |
| Help | `window-help` | 520×420 | Dialog card, fixed | Opened from tray/Settings; Close → destroy |

- Windows are **not** created in the builder; they are created by
  `open_surface` the first time the user activates a surface, and destroyed on
  close. Reopening creates a fresh window (fast: shared WebView2/WebKit
  runtime process).
- Surface open/close actions keep flowing through the existing
  `AppAction::ShowSurface`/`ToggleSurface` dispatcher so tray, hotkeys and
  commands all route identically.
- **Idle-RAM guardrail (must verify empirically):** after the last webview
  window is destroyed, confirm the WebView2 runtime processes
  (`msedgewebview2.exe`) are reaped. If Tauri keeps the WebView2 environment
  resident, the mitigation is lazy-initializing the webview environment only
  on the first `open_surface` (measure: Rust daemon stays < 15 MB; record the
  measured idle footprint in the implementation plan).

### 3.3 Repo layout

```
frontend/                      # NEW — Vite + React + TS + Tailwind + shadcn
  src/{mixer,settings,help}/   # 3 entry bundles (index.html per surface)
  src/lib/ipc.ts               # typed invoke/listen wrappers
  src/components/ui/           # shadcn/ui components (generated)
src-tauri/                     # NEW — tauri.conf.json, capabilities, binary host
  src/main.rs                  # tauri Builder + setup (WindowManager, native re-home)
  src/window_manager.rs
  src/commands.rs
crates/volumectl/              # unchanged core lib (audio, hotkeys, config, ui primitives)
```

The Tauri host binary depends on `volumectl_lib`; no core crate rewrite.

## 4. Rust side: commands, events, state

### 4.1 Tauri commands

- `get_bootstrap()` → `{ config, appearance_tokens, hotkey_status, volume }`
  (one-shot on webview mount)
- `adjust_volume(delta_pct)`, `set_volume(pct)`, `toggle_mute()`, `reset_volume()`
- `set_modifier(modifier)` — re-registers hotkeys via the existing path; emits
  `state://hotkeys`
- `save_config(partial)` — writes the existing `config.json` format
- `get_audio_sessions()` → per-app list `{ id, name, pct, muted, active }`
- `set_session_volume(id, pct)`, `mute_session(id)`
- `open_surface(id)`, `close_surface(id)` — WindowManager bridge

All commands return `Result<T, String>`; error messages surface as inline
toasts in the webview.

### 4.2 Events (Rust → webview)

- `state://volume` — `{ pct, muted }`, emitted on **every** volume change
  (hotkey, wheel, tray, slider), including the existing
  `publish_confirmed_state` path → open Mixer/Settings update live (< 2 ms
  target; same-process Tauri event channel).
- `state://hotkeys` — per-action `HotkeyRegStatus` (Registered/Conflicted),
  emitted after registration and after `set_modifier`.
- `state://sessions` — session list changes (device/app events, mute toggles).

### 4.3 Appearance resolution

Reuse the existing adaptive resolution
(`theme`/`material`/`motion`/`accent` + system theme + capability snapshot)
into a small token payload pushed with `get_bootstrap` and on change events.
The webview applies it via CSS custom properties + `data-theme` attributes
(Tailwind `dark:` variants), mirroring how the native surfaces consume the
same resolver today. One resolver, all surfaces — no drift.

## 5. Frontend

- Vite + React 19 + TypeScript; Tailwind v4; shadcn/ui (Radix primitives);
  lucide-react icons; framer-motion for entry/exit and layout animations.
- One Vite app, three HTML entry points (mixer/settings/help) → natural
  per-surface bundle splitting and lazy load.
- Typed IPC layer (`src/lib/ipc.ts`) wrapping `invoke` + `listen` so the UI
  never touches raw command strings.
- shadcn components used: Slider, Switch, Dialog, Card, Badge, Tabs, Input,
  Select, Tooltip, ScrollArea, Command (search), Kbd.

### 5.1 Mixer

- Per-app slider + quick mute button per row; framer-motion layout animation
  on reorder.
- Search filter (Command-style input) when many sessions.
- Auto-sort: sessions producing audio first, then by volume; stable ordering.
- Live updates via `state://volume`/`state://sessions`; slider drag writes via
  `set_session_volume` (Rust echoes back — no local-state drift).

### 5.2 Settings

- **Restricted hotkey recorder:** a modifier picker (Ctrl+Alt / Alt / Ctrl /
  CapsLock→Ctrl+Alt fallback with warning) rendered as a record-style control,
  plus read-only Kbd cards for the full combos (↑ ↓ Shift+↑ Shift+↓ M Shift+M R V).
- Step-size controls: small (default 1%) and large (default 10%) numeric inputs.
- Appearance controls: theme, material, motion, accent — same options as the
  current config, bound to `save_config`.
- **Conflict indicators:** red badge + tooltip on any action whose
  `HotkeyRegStatus` is `Conflicted` (from `state://hotkeys`); reflects the
  real registration outcome (e.g., external app holding Ctrl+Alt+M).

### 5.3 Help

- Card grid of all shortcuts (grouped: Volume / Commands), Kbd badges,
  searchable/filterable.

## 6. Data flow

1. **Bootstrap:** webview mounts → `invoke("get_bootstrap")` → renders →
   subscribes to `state://*`.
2. **Push (Rust → UI):** physical hotkey / wheel / tray →
   `apply_hotkey`/`handle_action` → `publish_confirmed_state` → also emits
   `state://volume` → open surfaces update (same-process channel, sub-ms).
3. **Pull (UI → Rust):** slider drag / toggle / form change → `invoke` →
   Rust mutates SSOT → emits the matching state event → every surface
   (including native HUD) stays consistent.
4. **Config:** settings form → `save_config` → Rust writes `config.json`
   (same schema/format) → `set_modifier` re-registers → `state://hotkeys`.

## 7. Error handling

- **WebView2 unavailable** (rare on Win10/11): `open_surface` returns an
  error; log a warning and show a tray notification; the core daemon (hotkeys,
  HUD, tray) keeps running — fail-soft by design.
- **IPC errors:** commands return `Result<T, String>`; webview shows an inline
  toast and keeps the last known state.
- **Hotkey conflicts:** existing resilient registration (skip + warn) is kept;
  Settings shows the conflict badge.
- **Config parse errors:** existing normalize/backup behavior is preserved.
- **Empty audio sessions / device changes:** `state://sessions` emits an empty
  list; Mixer renders an empty state instead of erroring.

## 8. Testing & CI

### 8.1 Rust

- Unit tests: WindowManager (lazy create/destroy, single-instance per surface,
  open/close routing), commands (state mutations, error mapping),
  AppearanceResolver (token payload correctness).
- Existing 252 tests stay green; cross-target checks (linux GTK + macOS)
  compile clean.

### 8.2 Frontend

- vitest + @testing-library/react:
  - Mixer: search filter, active-first sort, mute toggle, slider callback.
  - Settings: recorder display (modifier picker + key cards), conflict badge
    rendering from mocked `state://hotkeys`.
  - Help: filter behavior.
- No webview E2E in CI initially; a manual smoke checklist covers window
  lifecycle, sync latency (< 2 ms measurement), and idle RAM.

### 8.3 CI (`.github/workflows/ci.yml`)

- Add a **frontend build** step/job: Node 22, `npm ci`, `vite build`
  (type-checked), on the existing ubuntu job or a new `frontend` job.
- Ubuntu job: add the Tauri Linux system packages (user-specified):

  ```yaml
  - name: Install Linux & Tauri Dependencies (Ubuntu)
    if: runner.os == 'Linux'
    run: |
      sudo apt-get update
      sudo apt-get install -y \
        libwebkit2gtk-4.1-dev build-essential curl wget file \
        libxdo-dev libssl-dev libayatana-appindicator3-dev \
        librsvg2-dev pkg-config libx11-dev libasound2-dev
  ```

- Keep the 4-job matrix (checks / Windows / macOS / Ubuntu GTK); build with
  `tauri build --no-bundle` (or the equivalent cargo command) so artifacts
  stay single binaries; ship.sh updated to call the Tauri build and keep the
  enforcement battery intact.

## 9. Technical edge cases & remedies

### 9.1 Slider echo jitter (IPC race)

Dragging a Mixer slider at ~60 Hz fires `invoke("set_session_volume")` per
change; if the backend echoes `state://sessions`/`state://volume` back
immediately, the phase difference between the UI's in-flight drag value and
the echoed state makes the thumb jitter.

**Remedy (frontend):** optimistic local state + an `isDragging` ref; server
state is applied only while NOT dragging.

```tsx
// frontend/src/mixer/components/AppSlider.tsx (pattern)
export function AppSlider({ session }: { session: AudioSession }) {
  const [localVal, setLocalVal] = useState(session.pct);
  const isDragging = useRef(false);
  useEffect(() => { if (!isDragging.current) setLocalVal(session.pct); }, [session.pct]);
  const onChange = (v: number) => {
    setLocalVal(v); // immediate UI
    invoke("set_session_volume", { id: session.id, pct: v }); // async, no await
  };
  return <Slider value={[localVal]} onPointerDown={() => (isDragging.current = true)}
    onPointerUp={() => (isDragging.current = false)} onValueChange={([v]) => onChange(v)} />;
}
```

### 9.2 WebView2 idle-RAM leak

On Windows, after `destroy()`, the WebView2 runtime sub-processes
(`msedgewebview2.exe`) sometimes linger, pushing idle RAM past the < 15 MB
target.

**Remedy (host discipline + empirical verification):**

- `WindowManager::close_surface` destroys the webview window and removes it
  from the active-windows map in one place.
- Register a `tauri::WindowEvent::Destroyed` listener per surface to reap any
  per-window resources/event listeners; the WindowManager state is the only
  owner of webview handles.
- **Verify empirically in the plan:** after closing the last surface, confirm
  the WebView2 browser processes exit. If Tauri keeps the environment
  resident, lazy-init the WebView2 environment on first `open_surface` and
  measure the true idle footprint (Rust daemon alone must stay < 15 MB;
  record the measured numbers in the implementation plan).

```rust
// src-tauri/src/window_manager.rs (pattern)
impl WindowManager {
    pub fn close_surface(&self, app: &AppHandle, surface: SurfaceId) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(surface.as_label()) {
            window.destroy().map_err(|e| e.to_string())?;
            self.active_windows.lock().unwrap().remove(&surface);
        }
        Ok(())
    }
}
```

### 9.3 Wayland focus-loss & auto-close

`window-mixer` is frameless and auto-closes on losing focus. Browser-level
`blur` events are unreliable on Wayland (Hyprland/Sway/GNOME Wayland) for
transparent/frameless windows.

**Remedy:** close from the Rust `WindowEvent::Focused(false)` handler (reliable
on Win32 and Wayland compositors that send focus), and keep an Esc handler in
the React component as the always-works fallback.

```rust
// src-tauri/src/main.rs (pattern)
let app_handle = app.clone();
window.on_window_event(move |event| {
    if let tauri::WindowEvent::Focused(false) = event {
        let _ = app_handle.emit("close_mixer_request", ());
    }
});
```

**Platform caveat (honest):** Wayland per-pixel transparency is compositor-
dependent (xdg-shell has no standard alpha; Hyprland uses its own protocol).
The Mixer's transparent/acrylic look is best-effort on Wayland and falls back
to a solid themed background; X11 and Windows retain full transparency.

### 9.4 Tauri v2 capability & release-mode ACL gotchas

Tauri v2 ACL (default-deny) gates **core and plugin** APIs, not custom
`#[tauri::command]`s. The classic release-mode silent failures are: (a) the
surfaces use core window APIs (`close`, `hide`, `show`, `destroy`) that DO
need capability permissions; (b) a strict CSP blocks inline scripts/assets
— this directly interacts with the FOUC fix in §9.5 (an inline theme script
needs `script-src 'unsafe-inline'` or a nonce in `tauri.conf.json`'s CSP);
(c) wrong asset paths after bundling.

**Remedy:** one capability file covering the three surface windows, listing
`core:default` + the window permissions the surfaces actually use; CSP tuned
to allow the pre-hydrate theme script; asset paths verified by the CI
`--no-bundle` build.

```json
// src-tauri/capabilities/default.json (pattern)
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default-capability",
  "description": "Allow core commands for interactive surfaces",
  "windows": ["window-mixer", "window-settings", "window-help"],
  "permissions": [
    "core:default",
    "core:window:allow-close",
    "core:window:allow-destroy",
    "core:window:allow-hide",
    "core:window:allow-show"
  ]
}
```

### 9.5 FOUC (theme flash) with Tailwind v4

Tailwind v4 reads theme tokens from CSS (`@theme`/`data-theme`). If React waits
for `get_bootstrap` before applying the theme, every surface flashes
white/dark on open.

**Remedy:** apply the cached theme synchronously in each entry `index.html`
`<head>` before React mounts (cache from localStorage, seeded by the first
bootstrap). Must be paired with the CSP allowance from §9.4.

```html
<!-- frontend/src/mixer/index.html (pattern) -->
<script>
  const cached = localStorage.getItem("app-theme") || "dark";
  document.documentElement.setAttribute("data-theme", cached);
  if (cached === "dark") document.documentElement.classList.add("dark");
</script>
```

### 9.6 Stale audio-session race

The user drags a Mixer row for an app that closes / loses its audio device
mid-drag.

**Remedy (fail-soft):** `set_session_volume` returns `Result<(), String>`; a
stale `session_id` returns a lightweight error (no panic) and the backend
re-emits `state://sessions` so the frontend removes the dead row and shows a
brief inline notice instead of erroring.

## 10. Out of scope (YAGNI)

- Free-form per-action keybinding (config schema stays fixed).
- Installers / code signing / auto-updater (single-binary only for now).
- Overlay HUD rewrite (stays native).
- Webview E2E test infrastructure.
- Mobile targets.

## 11. Rollout

1. Scaffold `frontend/` (Vite + React + TS + Tailwind + shadcn) and
   `src-tauri/` (empty windows); prove a webview window opens from a native
   hotkey and closes cleanly with the idle-RAM measurement.
2. Ship the Mixer surface (sessions, sliders, mute, search, sort, live sync).
3. Ship Settings (recorder, steps, appearance, conflict badges).
4. Ship Help.
5. CI + ship.sh wiring; full battery; manual smoke; merge.
