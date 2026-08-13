# Hybrid Tauri UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-implement the Mixer, Settings and Help surfaces of `volume-control` as Tauri v2 webview windows (React 19 + TypeScript + Tailwind v4 + shadcn/ui) while keeping the native HUD overlay, tray, hotkeys and audio core unchanged.

**Architecture:** The Tauri app process hosts the whole app: native pieces (overlay/tray/wheel/hotkeys/audio) run in-process via a cross-platform `AppCore` SSOT; three webview windows (window-mixer, window-settings, window-help) are lazy-created on demand by a `WindowManager` and destroyed on close. Rust pushes `state://*` events to open surfaces; webviews pull via typed Tauri commands.

**Tech Stack:** Tauri v2 (`tauri` 2, `tauri-build` 2), React 19, TypeScript, Vite, Tailwind CSS v4, shadcn/ui (Radix), lucide-react, framer-motion, vitest + @testing-library/react, `global-hotkey` 0.8 (kept, not tauri-plugin-global-shortcut).

## Global Constraints

- **Spec:** `docs/superpowers/specs/2026-08-13-tauri-ui-hybrid-design.md` (read it first; the plan implements §3–§9 of it).
- **Hybrid boundary (D1):** Overlay HUD, tray, wheel hook, global-hotkey and audio core stay native. Only Mixer/Settings/Help become webview.
- **Window model (D2):** 3 separate webview windows, lazy-create on demand, destroy on close. Labels `window-mixer` (420×580, frameless transparent), `window-settings` (680×520, decorated resizable), `window-help` (520×420, dialog fixed). No windows created in the Tauri builder.
- **Frontend stack (D3):** React 19 + TypeScript + Vite + Tailwind v4 + shadcn/ui + lucide-react + framer-motion. One Vite app, three HTML entry points (`src/mixer/index.html`, `src/settings/index.html`, `src/help/index.html`).
- **Recorder (D4):** Restricted — only the configurable modifier (CtrlAlt/Alt/Ctrl/CapsLock→Ctrl+Alt fallback with warning); fixed keys (↑ ↓ Shift+↑ Shift+↓ M Shift+M R V) are read-only key cards. No config-schema change.
- **Packaging (D5):** `tauri build --no-bundle`; single portable binary; ship.sh keeps the full enforcement battery. Identifier is `dev.volumecontrol` (plan value `dev.volumecontrol.app` changed by user decision B on 2026-08-13 to avoid the Tauri config warning about identifiers ending in `.app`).
- **Appearances (D6):** Existing `theme/material/motion/accent` + adaptive resolution preserved; pushed to webviews as tokens (CSS variables + `data-theme`).
- **Hotkeys (D7):** Keep `hotkeys_global` + `global-hotkey` 0.8. Registration stays resilient (skip + warn); conflicts surface via `HotkeyRegStatus`.
- **Edge cases (§9):** slider echo-jitter (optimistic state + `isDragging` ref); WebView2 idle-RAM reaping (destroy + HashMap cleanup + `Destroyed` listener + empirical measurement); Wayland focus-loss (`WindowEvent::Focused(false)` + Esc fallback; Wayland transparency best-effort); capability file + CSP allowing the inline theme script; FOUC prevention (inline `<head>` script + localStorage cache); stale-session fail-soft (`Result<(), String>` + re-emit `state://sessions`).
- **Platform parity:** Windows + Linux + macOS all build and run. Per-app Mixer sessions are Windows-only (WASAPI); Linux/macOS show master volume + a "per-app mixing is Windows-only" empty state.
- **Quality gate (per commit):** `cargo fmt --all --check`, `git diff --check`, `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`, `cargo test --workspace --no-default-features`. Frontend: `npm run build` (tsc + vite) and `npm test` (vitest). Never weaken `-D warnings`.
- **Records guardrail:** every commit touching code must stage `feature_list.json` (vol-030) + `claude-progress.md` (Session 040) in the same commit; `scripts/check-records.sh --staged` must pass; pre-commit hook enforces.
- **CI:** keep the 4-job matrix; ubuntu job gains the webkit2gtk system packages from spec §8.3; add a frontend build step (Node 22, `npm ci`, `npm run build`).
- **Windows host:** use Git Bash; use the edit tool for tracked `.md`/`.json` (never PowerShell Set-Content); `frontend/` files (new, untracked) may be written by any tool.

---

### Task 1: Scaffold — frontend shell + src-tauri shell + WindowManager

**Files:**
- Create: `frontend/package.json`, `frontend/vite.config.ts`, `frontend/tsconfig.json`, `frontend/index.html` (root dev entry), `frontend/src/mixer/index.html`, `frontend/src/settings/index.html`, `frontend/src/help/index.html`, `frontend/src/main.tsx` (per-entry bootstrap), `frontend/src/lib/ipc.ts`, `frontend/src/styles.css` (Tailwind v4 `@import "tailwindcss"`), `frontend/src/components/ui/slider.tsx` (shadcn Slider), `frontend/.gitignore`
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/main.rs`, `src-tauri/src/window_manager.rs`, `src-tauri/src/lib.rs`
- Modify: `Cargo.toml` (add `src-tauri` to workspace members), `.gitignore` (add `frontend/node_modules`, `frontend/dist`, `src-tauri/target`, `src-tauri/gen`)

**Interfaces:**
- Consumes: nothing (first task).
- Produces:
  - `src-tauri/src/window_manager.rs`:
    ```rust
    pub enum SurfaceId { Mixer, Settings, Help }
    impl SurfaceId {
        pub fn label(&self) -> &'static str;   // "window-mixer" | "window-settings" | "window-help"
        pub fn entry(&self) -> &'static str;   // "src/mixer/index.html" | "src/settings/index.html" | "src/help/index.html" (Vite preserves input paths under dist/)
        pub fn all() -> [SurfaceId; 3];
    }
    pub struct WindowManager { app: tauri::AppHandle, active: Mutex<HashSet<SurfaceId>> }
    impl WindowManager {
        pub fn new(app: tauri::AppHandle) -> Self;
        pub fn open(&self, surface: SurfaceId) -> Result<(), String>;
        pub fn close(&self, surface: SurfaceId) -> Result<(), String>;
        pub fn close_all(&self) -> Result<(), String>;
        pub fn is_open(&self, surface: SurfaceId) -> bool;
    }
    ```
  - `src-tauri/src/lib.rs`: `#[cfg_attr(mobile, tauri::mobile_entry_point)] pub fn run() -> tauri::Result<()>;` plus `pub fn builder() -> tauri::Builder<tauri::Wry>` used by both `run()` and tests.
  - `frontend/src/lib/ipc.ts`: `export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;` and `export function listen<T>(event: string, handler: (payload: T) => void): Promise<() => void>;` (thin typed wrappers over `@tauri-apps/api`).

- [ ] **Step 1: Create the frontend scaffold**

```bash
cd frontend
npm init -y
npm install react react-dom @tauri-apps/api
npm install -D typescript vite @vitejs/plugin-react tailwindcss @tailwindcss/vite \
  @types/react @types/react-dom vitest @testing-library/react @testing-library/jest-dom \
  @tauri-apps/cli clsx tailwind-merge class-variance-authority lucide-react framer-motion
npm install @radix-ui/react-slider @radix-ui/react-switch @radix-ui/react-dialog \
  @radix-ui/react-select @radix-ui/react-tabs @radix-ui/react-tooltip
```

- [ ] **Step 2: Write `frontend/package.json` scripts** (replace the generated one)

```json
{
  "name": "volumecontrol-frontend",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "test": "vitest run"
  }
}
```

- [ ] **Step 3: Write the Vite/TS/Tailwind config**

`frontend/vite.config.ts` — multi-entry build (three HTML entry points), base `./` (Tauri serves from the app dir), dev server fixed port 1420:

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "node:path";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: "./",
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    rollupOptions: {
      input: {
        root: resolve(__dirname, "index.html"),
        mixer: resolve(__dirname, "src/mixer/index.html"),
        settings: resolve(__dirname, "src/settings/index.html"),
        help: resolve(__dirname, "src/help/index.html"),
      },
    },
  },
  test: { environment: "jsdom" },
});
```

`frontend/tsconfig.json` — `"jsx": "react-jsx"`, `"moduleResolution": "bundler"`, `"strict": true`, `"types": ["vitest/globals", "@testing-library/jest-dom"]`.

- [ ] **Step 4: Write the three HTML entries with the FOUC inline theme script (§9.5)**

`frontend/src/mixer/index.html` (settings/help identical except the `<title>`):

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Volume Mixer</title>
    <script>
      const cached = localStorage.getItem("app-theme") || "dark";
      document.documentElement.setAttribute("data-theme", cached);
      if (cached === "dark") document.documentElement.classList.add("dark");
    </script>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="./mixer.tsx"></script>
  </body>
</html>
```

- [ ] **Step 5: Write the per-entry bootstrap modules and a placeholder surface**

`frontend/src/mixer/mixer.tsx`:

```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { MixerSurface } from "./MixerSurface";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <MixerSurface />
  </StrictMode>,
);
```

`frontend/src/mixer/MixerSurface.tsx` (placeholder; real UI lands in Task 3):

```tsx
export function MixerSurface() {
  return <main className="p-4 text-sm">Mixer placeholder</main>;
}
```

Mirror this for `settings/SettingsSurface.tsx` and `help/HelpSurface.tsx`.

- [ ] **Step 6: Write `frontend/src/lib/ipc.ts`**

```ts
import { invoke as tauriInvoke, listen as tauriListen } from "@tauri-apps/api/core";

export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return tauriInvoke<T>(cmd, args);
}

export function listen<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
  return tauriListen<T>(event, handler);
}
```

- [ ] **Step 7: Write `frontend/src/styles.css` + shadcn Slider**

```css
@import "tailwindcss";

@custom-variant dark (&:where(.dark, .dark *));

:root {
  --background: 0 0% 100%;
  --foreground: 240 10% 3.9%;
  --accent: 199 100% 50%;
}
.dark {
  --background: 240 10% 3.9%;
  --foreground: 0 0% 98%;
}
```

Add a minimal shadcn-style `Slider` (Radix) in `frontend/src/components/ui/slider.tsx` (forwardRef, `data-slot` attributes, Tailwind classes; full shadcn output is fine to paste from `npx shadcn@latest add slider`).

- [ ] **Step 8: Write the src-tauri crate**

`src-tauri/Cargo.toml`:

```toml
[package]
name = "volumecontrol-tauri"
version = "0.1.0"
edition = "2021"

[lib]
name = "volumecontrol_tauri_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["tray-icon", "image-png"] }
tauri-plugin-opener = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
volumectl = { path = "../crates/volumectl" }
```

`src-tauri/build.rs`: `fn main() { tauri_build::build() }`

`src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "VolumeControl",
  "version": "0.1.0",
  "identifier": "dev.volumecontrol",
  "build": {
    "beforeDevCommand": "npm run dev --prefix ../frontend",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build --prefix ../frontend",
    "frontendDist": "../frontend/dist"
  },
  "app": {
    "windows": [],
    "security": {
      "csp": "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:"
    }
  },
  "bundle": { "active": false }
}
```

`src-tauri/capabilities/default.json` — exactly the file from spec §9.4 (windows `["window-mixer", "window-settings", "window-help"]`, permissions `core:default` + `core:window:allow-close|allow-destroy|allow-hide|allow-show`).

- [ ] **Step 9: Write `window_manager.rs` with a unit test**

```rust
use std::collections::HashSet;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceId { Mixer, Settings, Help }

impl SurfaceId {
    pub fn label(&self) -> &'static str {
        match self { Self::Mixer => "window-mixer", Self::Settings => "window-settings", Self::Help => "window-help" }
    }
    pub fn entry(&self) -> &'static str {
        match self { Self::Mixer => "src/mixer/index.html", Self::Settings => "src/settings/index.html", Self::Help => "src/help/index.html" }
    }
    pub fn all() -> [SurfaceId; 3] { [Self::Mixer, Self::Settings, Self::Help] }
    pub fn from_label(label: &str) -> Option<Self> {
        Self::all().into_iter().find(|s| s.label() == label)
    }
}

pub struct WindowManager {
    app: AppHandle,
    active: Mutex<HashSet<SurfaceId>>,
}

impl WindowManager {
    pub fn new(app: AppHandle) -> Self {
        Self { app, active: Mutex::new(HashSet::new()) }
    }

    pub fn is_open(&self, surface: SurfaceId) -> bool {
        self.active.lock().unwrap().contains(&surface)
    }

    pub fn open(&self, surface: SurfaceId) -> Result<(), String> {
        if self.is_open(surface) { return Ok(()); }
        let mut builder = WebviewWindowBuilder::new(&self.app, surface.label(), WebviewUrl::App(surface.entry().into()));
        match surface {
            SurfaceId::Mixer => {
                builder = builder
                    .inner_size(420.0, 580.0)
                    .decorations(false)
                    .transparent(true)
                    .always_on_top(true)
                    .skip_taskbar(true);
            }
            SurfaceId::Settings => {
                builder = builder.inner_size(680.0, 520.0).resizable(true);
            }
            SurfaceId::Help => {
                builder = builder.inner_size(520.0, 420.0).resizable(false);
            }
        }
        let window = builder.build().map_err(|e| e.to_string())?;
        let app_handle = self.app.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::Focused(false) = event {
                if surface == SurfaceId::Mixer {
                    let _ = app_handle.emit("close_mixer_request", ());
                }
            }
        });
        self.active.lock().unwrap().insert(surface);
        Ok(())
    }

    pub fn close(&self, surface: SurfaceId) -> Result<(), String> {
        if let Some(window) = self.app.get_webview_window(surface.label()) {
            window.destroy().map_err(|e| e.to_string())?;
        }
        self.active.lock().unwrap().remove(&surface);
        Ok(())
    }

    pub fn close_all(&self) -> Result<(), String> {
        for surface in SurfaceId::all() { self.close(surface)?; }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_labels_and_entries_are_stable() {
        assert_eq!(SurfaceId::Mixer.label(), "window-mixer");
        assert_eq!(SurfaceId::Mixer.entry(), "src/mixer/index.html");
        assert_eq!(SurfaceId::Settings.label(), "window-settings");
        assert_eq!(SurfaceId::Help.entry(), "help.html");
    }

    #[test]
    fn from_label_round_trips() {
        assert_eq!(SurfaceId::from_label("window-mixer"), Some(SurfaceId::Mixer));
        assert_eq!(SurfaceId::from_label("nope"), None);
    }
}
```

- [ ] **Step 10: Write `main.rs`/`lib.rs` with a demo command that opens/closes surfaces**

`src-tauri/src/lib.rs`:

```rust
use tauri::{Manager, State};
use window_manager::{SurfaceId, WindowManager};

mod window_manager;

pub fn builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![open_surface, close_surface])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            app.manage(WindowManager::new(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![open_surface, close_surface])
        .run(tauri::generate_context!())
}

#[tauri::command]
fn open_surface(window_manager: State<'_, WindowManager>, surface: String) -> Result<(), String> {
    let surface = SurfaceId::from_label(&surface).ok_or("unknown surface")?;
    window_manager.open(surface)
}

#[tauri::command]
fn close_surface(window_manager: State<'_, WindowManager>, surface: String) -> Result<(), String> {
    let surface = SurfaceId::from_label(&surface).ok_or("unknown surface")?;
    window_manager.close(surface)
}
```

- [ ] **Step 11: Add `src-tauri` to the workspace**

`Cargo.toml`: `members = ["crates/volumectl", "src-tauri"]`, `default-members = ["crates/volumectl", "src-tauri"]`. Run `cargo check --workspace` — it must pass (fix any Send/Sync or feature issues).

- [ ] **Step 12: Run the tests**

```bash
cargo test -p volumecontrol-tauri       # window_manager unit tests PASS
npm test --prefix frontend              # vitest placeholder PASS (add one smoke test for MixerSurface render)
npm run build --prefix frontend         # tsc + vite multi-entry build PASS
```

- [ ] **Step 13: Verify a webview window opens and closes, and measure idle RAM**

```bash
npm run dev --prefix frontend &                 # dev server on 1420
cargo tauri dev --no-watch                     # dev build; then from the shell of a Tauri window run:
#   invoke('open_surface', { surface: 'window-mixer' })  → window appears
#   invoke('close_surface', { surface: 'window-mixer' }) → window disappears
# Record: does the window open transparent + frameless? Close cleanly?
# Verify the dist layout matches SurfaceId::entry(): after `npm run build --prefix frontend`,
#   `ls frontend/dist/src/mixer/index.html` must exist (Vite preserves input paths under dist/).
# Idle RAM: with ALL webview windows closed, measure the process set:
#   PowerShell: (Get-Process -Name VolumeControl).WorkingSet64 / 1MB
# Record the number in the task report (target: Rust daemon < 15 MB, WebView2 runtime reaped).
```

- [ ] **Step 14: Commit**

```bash
git add Cargo.toml .gitignore frontend/ src-tauri/ feature_list.json claude-progress.md
git commit -m "feat: scaffold Tauri v2 shell with lazy WindowManager (3 webview surfaces)"
```

Records: add `feature_list.json` vol-030 (status `in-progress`, verification = the Task 1 commands) and a `claude-progress.md` Session 040 section before committing.

---

### Task 2: AppCore (SSOT) + IPC commands + state events

**Files:**
- Create: `crates/volumectl/src/host_core.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/events_sink.rs`
- Modify: `crates/volumectl/src/lib.rs` (`pub mod host_core;`), `crates/volumectl/src/audio/mod.rs` (add `Send + Sync` bounds if missing), `src-tauri/src/lib.rs` (register commands + events sink + hotkey poll task), `src-tauri/Cargo.toml`

**Interfaces:**
- Consumes: `window_manager::{SurfaceId, WindowManager}` (Task 1); `volumectl::hotkeys_global::GlobalHotkeys`; `volumectl::audio::AudioBackend`; `volumectl::ui::{AppAction, SurfaceId as UiSurfaceId}`; `volumectl::hotkeys::{HotkeyAction, HotkeyRegResult}`; `volumectl::config::{Config, HotkeyModifier}`.
- Produces (`crates/volumectl/src/host_core.rs`):
  ```rust
  pub struct AudioSessionInfo { pub id: String, pub name: String, pub pct: u8, pub muted: bool, pub active: bool }
  #[derive(serde::Serialize)]
  pub struct AppearancePayload {
      pub theme_resolved: String, // "dark" | "light"
      pub material: String,       // "Auto" | "Opaque" | "Translucent"
      pub motion: String,         // "Full" | "Reduced"
      pub accent: String,         // "System" | "Blue" | ...
  }
  pub struct BootstrapPayload {
      pub config: Config,
      pub volume_pct: u8,
      pub muted: bool,
      pub hotkey_status: Vec<HotkeyRegResult>,
      pub appearance: AppearancePayload,
      pub sessions: Vec<AudioSessionInfo>,
      pub sessions_supported: bool,
  }
  pub trait EventSink: Send + Sync {
      fn volume(&self, pct: u8, muted: bool);
      fn hotkeys(&self, status: &[HotkeyRegResult]);
      fn sessions(&self, sessions: &[AudioSessionInfo]);
  }
  pub struct AppCore { /* config, audio, hotkeys, last_state, hotkey_status, sink, modifier */ }
  impl AppCore {
      pub fn new(audio: Box<dyn AudioBackend>, config: Config, modifier: HotkeyModifier, sink: Arc<dyn EventSink>) -> Result<Self, String>;
      pub fn bootstrap(&mut self) -> BootstrapPayload;
      pub fn poll_hotkeys(&mut self) -> Option<HotkeyAction>; // drains GlobalHotkeys channel
      pub fn apply_hotkey(&mut self, action: HotkeyAction);
      pub fn handle_action(&mut self, action: AppAction);
      pub fn publish_confirmed_state(&mut self);
      pub fn set_modifier(&mut self, modifier: HotkeyModifier) -> Result<(), String>;
      pub fn save_config(&mut self) -> Result<(), String>;
      pub fn sessions(&mut self) -> Vec<AudioSessionInfo>;
      pub fn set_session_volume(&mut self, id: &str, pct: u8) -> Result<(), String>;
      pub fn mute_session(&mut self, id: &str) -> Result<(), String>;
  }
  ```
  - `AppAction`/`HotkeyAction`/`SurfaceId` come from existing modules; `AppCore` maps `SurfaceId::Mixer` to `WindowManager` via the sink's `open_surface`/`close_surface` commands (the Tauri host routes these).
  - Event payload types (`HotkeyRegResult`, `Config`, `AudioSessionInfo`, `AppearancePayload`) must derive `serde::Serialize`; add the derive to `HotkeyRegResult` in `crates/volumectl/src/hotkeys/mod.rs` if it does not already have it.

- [ ] **Step 1: Write the failing test for AppCore state + events**

`crates/volumectl/tests/host_core.rs`:

```rust
use std::sync::{Arc, Mutex};
use volumectl::audio::{AudioBackend, VolumeState};
use volumectl::config::{Config, HotkeyModifier};
use volumectl::host_core::{AppCore, AudioSessionInfo, EventSink};
use volumectl::hotkeys::HotkeyRegResult;

struct StubAudio { state: VolumeState }
impl AudioBackend for StubAudio {
    fn get_state(&self) -> Result<VolumeState, volumectl::audio::AudioError> { Ok(self.state) }
    fn set_volume(&self, volume: f32) -> Result<(), volumectl::audio::AudioError> { Ok(()) }
    fn toggle_mute(&self) -> Result<VolumeState, volumectl::audio::AudioError> { Ok(self.state) }
    fn set_mute(&self, _muted: bool) -> Result<(), volumectl::audio::AudioError> { Ok(()) }
}

#[derive(Default)]
struct RecordingSink { events: Mutex<Vec<String>> }
impl EventSink for RecordingSink {
    fn volume(&self, pct: u8, muted: bool) {
        self.events.lock().unwrap().push(format!("volume:{pct}:{muted}"));
    }
    fn hotkeys(&self, _status: &[HotkeyRegResult]) {}
    fn sessions(&self, _sessions: &[AudioSessionInfo]) {}
}

#[test]
fn adjust_volume_emits_volume_event() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = AppCore::new(
        Box::new(StubAudio { state: VolumeState { volume: 0.5, muted: false } }),
        Config::default(),
        HotkeyModifier::CtrlAlt,
        sink.clone(),
    )
    .unwrap();
    core.apply_hotkey(volumectl::hotkeys::HotkeyAction::VolumeUp);
    assert!(sink.events.lock().unwrap().iter().any(|e| e.starts_with("volume:51:")));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p volumectl --test host_core`
Expected: FAIL — `host_core` module and `AppCore` not found.

- [ ] **Step 3: Implement `host_core.rs`**

Extract from the Windows `app.rs` `AppContext`: keep `handle_action`/`apply_hotkey`/`publish_confirmed_state`/blacklist gate/beep/appearance resolution behavior, but replace the win32 surface plumbing with `EventSink` calls + the existing `ui::AppAction` dispatcher. Provide the struct + methods above with `pub(crate)` free helpers `step_volume` (already in `core`), `hotkey_to_action` (move from `app.rs` — it is already cross-platform pure code). Session enumeration on Windows reuses the WASAPI code path used by the current mixer; on Linux/macOS `sessions()` returns `vec![]` and `sessions_supported = false`. `set_session_volume`/`mute_session` return `Ok(())` on unsupported platforms (no-op) and `Err(String)` with a re-emitted `sessions` event on Windows when the `id` is stale (spec §9.6).

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p volumectl --test host_core`
Expected: PASS.

- [ ] **Step 5: Implement `events_sink.rs` (Tauri side)**

```rust
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use volumectl::host_core::{AudioSessionInfo, EventSink};
use volumectl::hotkeys::HotkeyRegResult;

pub struct TauriSink { app: AppHandle }

impl TauriSink {
    pub fn new(app: AppHandle) -> Self { Self { app } }
}

impl EventSink for TauriSink {
    fn volume(&self, pct: u8, muted: bool) {
        let _ = self.app.emit("state://volume", serde_json::json!({ "pct": pct, "muted": muted }));
    }
    fn hotkeys(&self, status: &[HotkeyRegResult]) {
        let _ = self.app.emit("state://hotkeys", status);
    }
    fn sessions(&self, sessions: &[AudioSessionInfo]) {
        let _ = self.app.emit("state://sessions", sessions);
    }
}
```

- [ ] **Step 6: Implement `commands.rs`** — all commands from spec §4.1, each `pub fn` annotated `#[tauri::command]`, taking `State<'_, Mutex<AppCore>>` (AppCore is managed as `Mutex<AppCore>` because commands need `&mut`), plus `State<'_, WindowManager>` for `open_surface`/`close_surface`. Wire `Arc<TauriSink>` into `AppCore::new` in the Builder setup. Return `Result<T, String>` everywhere; map audio errors with `.map_err(|e| e.to_string())`.

- [ ] **Step 7: Wire the hotkey poll task in `src-tauri/src/lib.rs`**

```rust
// in setup(), after manage():
let core = app.state::<Mutex<AppCore>>();
let hotkeys_handle = core.clone();
std::thread::spawn(move || loop {
    if let Some(action) = hotkeys_handle.lock().unwrap().poll_hotkeys() {
        // poll_hotkeys already applied the action inside AppCore (emit + publish)
        let _ = action;
    }
    std::thread::sleep(std::time::Duration::from_millis(20));
});
```

(`AppCore::poll_hotkeys` drains the `GlobalHotkeys` channel, calls `apply_hotkey` for each event and returns the last action for logging; it never blocks.)

- [ ] **Step 8: Tests — command error mapping + event emission**

- `crates/volumectl/tests/host_core.rs`: a second test asserting `set_session_volume` on an unsupported platform returns `Ok(())` and that a volume mutation through `handle_action(AppAction::AdjustVolume{..})` produces a `"volume:..."` sink event.
- `src-tauri/src/commands.rs` unit tests (no app instance): pure argument mapping helpers (`SurfaceId::from_label` reuse).
Run: `cargo test --workspace --no-default-features` — all pass (existing 252 + new).

- [ ] **Step 9: Manual end-to-end check**

```bash
npm run dev --prefix frontend &
cargo tauri dev --no-watch
```
In the mixer window devtools console:
```js
const b = await window.__TAURI__.core.invoke('get_bootstrap');
console.log(b.volume_pct, b.muted, b.hotkey_status);
```
Then press the physical `Ctrl+Alt+↑` — confirm a `state://volume` event arrives in < 2 ms (devtools → console log from a `listen('state://volume')` subscriber) and `b.volume_pct` increases by 1.

- [ ] **Step 10: Commit**

```bash
git add crates/volumectl/src/host_core.rs crates/volumectl/tests/host_core.rs \
  crates/volumectl/src/lib.rs src-tauri/src/commands.rs src-tauri/src/events_sink.rs \
  src-tauri/src/lib.rs feature_list.json claude-progress.md
git commit -m "feat: AppCore SSOT with Tauri commands and state:// events"
```

---

### Task 3: Mixer webview surface

**Files:**
- Create: `frontend/src/mixer/MixerSurface.tsx`, `frontend/src/mixer/AppSlider.tsx`, `frontend/src/mixer/SessionRow.tsx`, `frontend/src/mixer/sessionStore.ts`, `frontend/src/mixer/MixerSurface.test.tsx`
- Modify: `frontend/src/mixer/mixer.tsx` (render real surface)

**Interfaces:**
- Consumes: `invoke<T>`/`listen<T>` from `frontend/src/lib/ipc.ts`; `BootstrapPayload` shape (Task 2): `{ volume_pct, muted, hotkey_status, appearance, sessions, sessions_supported }`; `state://volume` event `{ pct, muted }`; `state://sessions` event `AudioSessionInfo[]`; commands `get_bootstrap`, `set_session_volume(id, pct)`, `mute_session(id)`, `set_volume(pct)`, `toggle_mute()`, `close_surface(surface)`.
- Produces: none (leaf UI).

- [ ] **Step 1: Write the failing tests**

`frontend/src/mixer/MixerSurface.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MixerSurface } from "./MixerSurface";
import * as ipc from "../lib/ipc";

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "get_bootstrap") return {
      volume_pct: 55, muted: false,
      sessions: [
        { id: "a", name: "Spotify", pct: 40, muted: false, active: true },
        { id: "b", name: "Game", pct: 90, muted: false, active: false },
        { id: "c", name: "MutedApp", pct: 10, muted: true, active: false },
      ],
      sessions_supported: true, hotkey_status: [], appearance: {},
    };
    return {};
  }),
  listen: vi.fn(async () => () => {}),
}));

describe("MixerSurface", () => {
  it("renders sessions with active apps sorted first", async () => {
    render(<MixerSurface />);
    const rows = await screen.findAllByTestId("session-row");
    expect(rows[0]).toHaveTextContent("Spotify"); // active first
  });

  it("filters by search term", async () => {
    render(<MixerSurface />);
    await screen.findAllByTestId("session-row");
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: "spot" } });
    expect(screen.getAllByTestId("session-row")).toHaveLength(1);
  });

  it("toggles mute for a session", async () => {
    render(<MixerSurface />);
    const mute = (await screen.findAllByRole("button", { name: /mute/i }))[2];
    fireEvent.click(mute);
    expect(ipc.invoke).toHaveBeenCalledWith("mute_session", { id: "c" });
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm test --prefix frontend`
Expected: FAIL — `MixerSurface` doesn't exist.

- [ ] **Step 3: Implement `sessionStore.ts`** — a small hook `useSessions()` that loads `get_bootstrap` and subscribes to `state://sessions` + `state://volume`, applying sort (active first, then by pct desc) and the `isDragging` echo guard.

- [ ] **Step 4: Implement `AppSlider.tsx`** — exactly the §9.1 pattern (optimistic local state + `isDragging` ref), invoking `set_session_volume` without awaiting.

- [ ] **Step 5: Implement `SessionRow.tsx` + `MixerSurface.tsx`** — shadcn Slider + Switch/Button mute + search Input (Command-style) + empty state ("No audio sessions" / "Per-app mixing is Windows-only" when `sessions_supported === false`); `data-testid="session-row"`; Esc key handler that calls `close_surface({ surface: "window-mixer" })`; framer-motion `layout` animation on the list.

- [ ] **Step 6: Run tests to verify they pass**

Run: `npm test --prefix frontend`
Expected: PASS (3 tests).

- [ ] **Step 7: Manual check**

```bash
npm run dev --prefix frontend &  cargo tauri dev --no-watch
```
Open mixer → rows render; drag a slider (no jitter); press `Ctrl+Alt+↑` while mixer open → slider updates live; Esc closes the window; kill an app playing audio → its row disappears.

- [ ] **Step 8: Commit** (stage frontend files + records; message `feat: Mixer webview surface (search, sort, mute, live sync)`)

---

### Task 4: Settings webview surface

**Files:**
- Create: `frontend/src/settings/SettingsSurface.tsx`, `frontend/src/settings/ModifierPicker.tsx`, `frontend/src/settings/KeyCard.tsx`, `frontend/src/settings/SettingsSurface.test.tsx`, `frontend/src/settings/modifierOptions.ts`
- Modify: `frontend/src/settings/settings.tsx`

**Interfaces:**
- Consumes: `get_bootstrap` (`config.modifier`, `config.volume_step`, `config.volume_step_large`, `hotkey_status`, `appearance`), commands `set_modifier(modifier)`, `save_config(partial)`, `close_surface(surface)`; events `state://hotkeys`, `state://volume`; `HotkeyRegStatus` enum values `Registered`/`Conflicted`.
- Produces: none.

- [ ] **Step 1: Write the failing tests** — `SettingsSurface.test.tsx`:
  1. renders the modifier picker with 3 options (Ctrl+Alt / Alt / Ctrl) + CapsLock shown disabled with a "falls back to Ctrl+Alt" tooltip;
  2. clicking a modifier calls `invoke("set_modifier", { modifier: "Alt" })`;
  3. a `Conflicted` status for `ToggleMute` renders a red badge with the error text;
  4. step-size inputs call `save_config` with `{ volume_step: 5 }`.
- [ ] **Step 2: Run tests to verify they fail** (`npm test --prefix frontend`)
- [ ] **Step 3: Implement `modifierOptions.ts`** — the 4 options with labels and Kbd combos for the fixed keys per modifier (Ctrl+Alt: `Ctrl+Alt+↑`, `Ctrl+Alt+Shift+↑`, `Ctrl+Alt+M`, `Ctrl+Alt+Shift+M`, `Ctrl+Alt+R`, `Ctrl+Alt+V`; Alt: `Alt+↑`...; Ctrl: `Ctrl+↑`...).
- [ ] **Step 4: Implement `KeyCard.tsx`** — shadcn Card + Kbd showing each full combo for the selected modifier, plus a red `Badge` when the action's `HotkeyRegStatus` is `Conflicted`.
- [ ] **Step 5: Implement `ModifierPicker.tsx` + `SettingsSurface.tsx`** — RadioGroup-style record picker, step-size numeric Inputs (small default 1, large default 10), appearance controls (theme/material/motion/accent Selects bound to `save_config`), conflict summary from `state://hotkeys`, Esc → `close_surface`.
- [ ] **Step 6: Run tests to verify they pass** (`npm test --prefix frontend` — 4 tests)
- [ ] **Step 7: Manual check** — change modifier to Alt → physical `Alt+↑` works and Settings reflects it; force a conflict (register Ctrl+Alt+M in another app) → badge appears.
- [ ] **Step 8: Commit** (message `feat: Settings webview surface (restricted recorder, steps, conflict badges)`)

---

### Task 5: Help webview surface

**Files:**
- Create: `frontend/src/help/HelpSurface.tsx`, `frontend/src/help/HelpSurface.test.tsx`, `frontend/src/help/shortcuts.ts`
- Modify: `frontend/src/help/help.tsx`

**Interfaces:**
- Consumes: `get_bootstrap` (`config.modifier`); command `close_surface`.
- Produces: none.

- [ ] **Step 1: Write the failing tests** — `HelpSurface.test.tsx`:
  1. renders all shortcut cards grouped by Volume / Commands;
  2. filters cards by a search box;
  3. each card shows a Kbd badge with the current modifier's combo.
- [ ] **Step 2: Run tests to verify they fail**
- [ ] **Step 3: Implement `shortcuts.ts`** — the fixed shortcut set keyed by modifier (same combos as Task 4), with labels (Volume Up/Down/Large, Toggle Mute, Open Menu, Reset 50%, Open Mixer, Settings, Help).
- [ ] **Step 4: Implement `HelpSurface.tsx`** — Card grid + search filter + Kbd badges; Esc → `close_surface`.
- [ ] **Step 5: Run tests to verify they pass** (`npm test --prefix frontend`)
- [ ] **Step 6: Manual check** — Help opens from the tray menu, filters, closes on Esc.
- [ ] **Step 7: Commit** (message `feat: Help webview surface (shortcut cards + filter)`)

---

### Task 6: Full host integration — native re-home + cross-platform main.rs

**Files:**
- Modify: `src-tauri/src/lib.rs` (setup: create audio backend + config + GlobalHotkeys + AppCore; install win32 overlay/tray/wheel on Windows; drain timers; `tauri::generate_context!` stays), `src-tauri/src/commands.rs`, `crates/volumectl/src/main.rs` (per-platform entry now delegates to the Tauri host on all platforms), `src-tauri/Cargo.toml` (Windows-only deps: `windows-sys` features; Linux: `gtk-renderer` feature reuse)
- Create: `src-tauri/src/native_win32.rs` (Windows: overlay + tray + wheel hook wiring, reusing `volumectl::overlay`/`tray`/`wheel_win32`), `src-tauri/src/native_headless.rs` (Linux/macOS: hotkeys + audio only)
- Delete (Windows-only, replaced by webview): `crates/volumectl/src/mixer.rs`, `crates/volumectl/src/settings.rs`, `crates/volumectl/src/help.rs` native window implementations — **only after the webview surfaces pass their manual checks (Tasks 3–5)**; keep `overlay.rs` + `tray.rs`.

**Interfaces:**
- Consumes: everything from Tasks 1–2; existing `volumectl::app` (Windows) re-homed: the `AppContext`-equivalent logic now lives in `host_core::AppCore`; the old `app.rs` message loop is replaced by the Tauri event loop + the poll task from Task 2.
- Produces: the production binary behaves exactly like today's app (hotkeys, HUD overlay, tray, wheel, blacklist, beep) plus the three webview surfaces; `crates/volumectl/src/main.rs` builds on all platforms via the Tauri host.

- [ ] **Step 1: Write the failing cross-target compile check** — `cargo check --workspace --no-default-features` on Linux/macOS target fails because `main.rs` still references the old native-only entry.
- [ ] **Step 2: Re-home the Windows native pieces** — move overlay/tray/wheel creation into `native_win32.rs`; AppCore drives them through the existing `ui::AppAction` dispatcher (keep the win32 window classes, but their wndprocs now only handle overlay/tray/wheel messages; mixer/settings/help messages are dropped).
- [ ] **Step 3: Rewire `crates/volumectl/src/main.rs`** — on every platform: `volumecontrol_tauri_lib::run()`; the Linux/macOS headless host logic (`linux_host_core`/`macos_app`) is superseded by `AppCore` + Tauri host (hotkeys + audio + webview surfaces; no CLI-only regression: keep `cli` commands working when an argument is present).
- [ ] **Step 4: Verify the full gate locally (Windows)** — `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`; `cargo test --workspace --no-default-features`; `npm run build --prefix frontend`; `npm test --prefix frontend`. Then `cargo check --target x86_64-unknown-linux-gnu` and `--target x86_64-apple-darwin` compile clean (pkg-config stub per windows-host skill for the Linux GTK cross-check).
- [ ] **Step 5: Manual regression** — the full manual smoke from the previous release still passes: 1% hotkey steps, hold-repeat, Shift large step (physical keyboard), M/R/V, wheel hook, tray, overlay HUD, idle CPU, plus: hotkey opens the webview mixer; idle RAM with all webviews closed measured and recorded (compare against the pre-Tauri baseline: daemon < 15 MB).
- [ ] **Step 6: Commit** (message `refactor: integrate native core into Tauri host (all platforms)`)

---

### Task 7: CI + ship.sh wiring

**Files:**
- Modify: `.github/workflows/ci.yml`, `scripts/ship.sh`, `scripts/ship.ps1`, `scripts/format-lint.sh` (if the gate needs the frontend build), `scripts/check-records.sh` (if frontend files need exempting from the records guard), `docs/global-hotkeys.md` (reference the webview surfaces)

**Interfaces:**
- Consumes: the built `frontend/dist` + `src-tauri` (Tasks 1–6).
- Produces: CI runs the frontend build + full enforcement battery; ship.sh builds the single binary via `tauri build --no-bundle`.

- [ ] **Step 1: Update `ci.yml`** — ubuntu job: add the spec §8.3 apt packages (webkit2gtk-4.1 etc.) and a `Frontend build` step (`npm ci` in `frontend/`, `npm run build`); windows/macos jobs: add `npm run build --prefix frontend` before `cargo build` (tauri's `beforeBuildCommand` also covers it, but CI should type-check explicitly).
- [ ] **Step 2: Update `ship.sh`/`ship.ps1`** — replace the cargo build command with `npm run build --prefix frontend && npx tauri build --no-bundle` (run from the repo root; `tauri.conf.json`'s `beforeBuildCommand` also builds the frontend, but ship builds it explicitly first so failures surface early). Keep every enforcement phase: records → fmt/whitespace → clippy → tests → smoke battery. The produced artifact is `src-tauri/target/release/VolumeControl(.exe)`.
- [ ] **Step 3: Verify the self-test battery still passes** — `bash scripts/test-check-records.sh`, `bash scripts/test-format-lint.sh`, `bash scripts/test-ship.sh` (update expected command strings if the ship smoke test asserts the build command).
- [ ] **Step 4: Commit** (message `ci: Tauri frontend build + webkit2gtk deps; ship via tauri build`)

---

### Task 8: Final verification + records + manual smoke

- [ ] **Step 1: Full battery** — `bash scripts/check-records.sh --branch`, `bash scripts/format-lint.sh`, `bash scripts/test-check-records.sh`, `bash scripts/test-format-lint.sh`, `bash scripts/test-ship.sh`, plus `npm test --prefix frontend` — all exit 0.
- [ ] **Step 2: Cross-target checks** — Linux GTK + macOS compile clean (pkg-config stub on Windows host).
- [ ] **Step 3: Manual smoke checklist** (record results in the task report):
  1. Hotkeys (1% steps, hold-repeat, Shift large, M/R/V) — no regression.
  2. Overlay HUD + tray + wheel — no regression.
  3. Mixer webview: open via hotkey, live sync < 2 ms, search, sort, mute, stale-row removal.
  4. Settings webview: modifier change re-registers hotkeys; conflict badge; step-size saves to config.
  5. Help webview: open from tray, filter, close.
  6. Idle RAM with all webviews closed (record number vs < 15 MB target) and WebView2 processes reaped (Windows).
  7. Idle CPU ~0%.
- [ ] **Step 4: Records** — `feature_list.json` vol-030 → `status: "passing"` with the exact commands run; `claude-progress.md` Session 040 verification section; `session-handoff.md` session/feature/test-count updates.
- [ ] **Step 5: Pre-push review** — run the mandatory three-domain pre-push review (guard core / gate chain / wiring-records), fix genuine defects, re-run the battery.
- [ ] **Step 6: Ship** — `bash scripts/ship.sh --push` (or the PowerShell bridge); verify GitHub Actions green (all 4 jobs); open the PR `feature/tauri-ui-hybrid` → `master`; merge when CI is green.
- [ ] **Step 7: Post-merge** — append the CI run URL to vol-030 evidence; clean up the SDD workspace; refresh `session-handoff.md`; final report.
