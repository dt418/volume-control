# Tauri → Egui Migration Plan (canonical)

**Role:** Senior Rust Architect & Native Desktop Migration Engineer (user-provided ROLE, 2026-10-24)
**Status:** Approved process — executing incrementally per ROLE execution rules
**Detailed annexes (superpowers flow):**
- Design spec: `docs/superpowers/specs/2026-10-24-egui-foundation-migration-design.md`
- Task plan (SDD+TDD): `docs/superpowers/plans/2026-10-24-egui-foundation-migration.md`
- Legacy blueprint: `docs/references/legacy-app/design-spec.md`
- Feature SSOT: `docs/references/legacy-app/feature-reference.md`
- Status tracker: [`FEATURE_PARITY.md`](FEATURE_PARITY.md) · Log: [`MIGRATION_LOG.md`](MIGRATION_LOG.md)

## 1. Architecture

**Before (current production):** Tauri 2 + WebView2 per surface (4 windows) + React/Tailwind frontend; Rust host `src-tauri` on `volumectl::host_core::AppCore`; IPC commands + events; plugins (notification, single-instance via custom mutex).

**After (target):** Single-process native app — `crates/volumecontrol-egui` on **eframe 0.36 (wgpu 30, accesskit)** with immediate child viewports; **no WebView, no JS runtime, no IPC** (UI calls `AppCore::apply_action` directly); same `AppCore` + `EventSink` seam (inverted: UI implements the sink); same `config.ini` SSOT; window-vibrancy for OS glass.

```text
Tauri:  [React UI] --IPC--> [src-tauri AppCore] --> volumectl
Egui:   [egui UI] --apply_action--> [AppCore] --EventSink(mpsc)--> [egui UI repaint]
```

**Module layout** (adapted ROLE template): `src/{main,app,style,i18n,sink,placement,vibrancy,cli,native_win32}.rs` + `src/surfaces/{mixer,settings,help,overlay,tray}.rs`; domain logic stays in `volumectl` (no rewrite).

## 2. Dependency map (Tauri → egui destination)

| Tauri integration | Legacy implementation | Egui destination |
|---|---|---|
| IPC commands (26, `src-tauri/src/commands.rs`) | invoke → AppCore | direct `AppCore::apply_action` / accessor calls (no IPC); full inventory read from `commands.rs` during Task 5 |
| Events `state://volume|sessions|update|hotkeys|overlay|backend` | emit → React listeners | `EventSink` methods → mpsc → repaint + state mirror |
| Windows ×4 + placement | `window_manager.rs` | eframe viewports (Mixer = root; `placement::frame_for`) |
| Tray + menu | `tray-icon`/`muda` (already Rust) | reused verbatim in `surfaces/tray.rs` (Task 5) |
| Global hotkeys | `global-hotkey` crate (already Rust) | reused verbatim (Task 5 wiring) |
| Single instance + show-mixer | `native_win32.rs` mutex/event | port to egui `native_win32.rs` (Task 5) |
| Autostart | `volumectl::autostart` | reused (Task 5) |
| Notifications (vol-116 toasts) | `tauri-plugin-notification` | **native replacement** — `tauri-winrt-notification` crate (already a dev-dep pattern in repo) or `windows` toast API; Task 5 |
| Update check/install | `ureq` + `UpdateManager` (Rust) | reused; UI = egui update card |
| Config | `config.ini` via `volumectl::config` | reused unchanged; **eframe persistence disabled** (SSOT) |
| CLI verbs get/set/mute | `src-tauri/src/main.rs` | port to `cli.rs` (Task 5) |
| Dialogs | `rfd`-equivalent (legacy used none beyond explorer open) | `OpenConfigLocation` → `explorer` spawn (existing) |
| Deep links / clipboard | none in legacy | none (recorded) |

## 3. Ordered stages (= ROLE phases)

| Stage | Content | Plan task |
|---|---|---|
| P0 Discovery | done — feature-reference + design-spec + AppCore audit | — |
| P1 Design | done — spec approved-pending | — |
| P2 Scaffold + state | crate, AppCore+EguiSink, i18n, style, viewport skeleton, vibrancy | Task 1 |
| P3 UI surfaces | Mixer, Settings 6 tabs, Help, Overlay (TDD, feature-reference rows) | Tasks 2–4 |
| P4 Tauri replacements | tray, single instance, CLI, wheel, notifications, autostart | Task 5 |
| P5 Data compat | config.ini unchanged (verify by fixture test); eframe persistence off | Task 1 + Task 6 |
| P6 Performance | benchmark table vs native host; repaint discipline | Task 6 |
| P7 Testing | workspace gates + parity sweep (computer use, row-by-row) | Task 6–7 |
| P8 Cleanup/delivery | retire old renderer, CI/packaging updates, docs, final report | Task 8 |

**Threading/ownership rule** (resolves the AppCore seam): the egui App owns
`AppCore` directly on the UI thread (no extra Mutex — eframe `ui()` is
single-threaded `&mut self`); `EguiSink` methods are **channel-only** — they
never call back into the core synchronously, so no re-entrancy or deadlock.

**Deletion ordering** (Task 8): `src/i18n.rs` parity test must pass BEFORE
`frontend/` is deleted (it is the i18n SSOT until the port is verified);
`crates/volumecontrol-host` (native crate + binary) is removed in full — the
egui crate becomes the host binary (final bin naming: keep
`volumecontrol-host`); `crates/dioxus-settings-prototype` removed.

## 4. Risks

See superpowers plan Risk Register (vibrancy HWND lookup, recorder capture, repaint coupling, glass bar) + added:
- **Hidden Mixer root may stall immediate children** — Task 1 spike; fallback: 1×1 transparent off-screen root (recorded before Task 2).
- **Windows toast notifications (vol-116)** — `tauri-winrt-notification` needs an AUMID/shortcut; fallback = in-app update banner only, recorded degradation.

## 5. Acceptance criteria (= ROLE Definition of Done)

Spec §9 gates + ROLE DoD: runs without Tauri; every `FEATURE_PARITY.md` row Implemented+Verified or an approved exception; user data/config accessible (fixture test); no UI freeze; gates green; release build validated on Windows; obsolete deps removed; docs updated; remaining limits reported. Final report follows the ROLE 10-section format (Task 8 deliverable).
