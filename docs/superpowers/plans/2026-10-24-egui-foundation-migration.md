# Egui Foundation Migration Implementation Plan (SDLC · SDD · TDD)

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development (controller → per-task implementer subagent → per-task reviewer subagent), with superpowers:test-driven-development governing every step. Steps use checkbox (`- [ ]`) syntax for tracking. Fallback if no subagent tool: superpowers:executing-plans with a written `Final review: self-review` note.

**Goal:** Rebuild VolumeControl on egui/eframe 0.36 with full v0.1.11 feature parity and a liquid-glass visual style on Windows, macOS, and Linux; cut over only after the parity sweep passes.

**Architecture:** New parallel binary crate `crates/volumecontrol-egui` (eframe multi-viewport, Mixer-as-root, immediate child viewports) sitting on the untouched `volumectl::host_core::AppCore` full reducer through an `EguiSink: EventSink` adapter (mpsc into the eframe loop). Old native host stays runnable as benchmark baseline until the cutover task.

**Tech Stack:** eframe/egui 0.36 (wgpu 30, accesskit default), window-vibrancy 0.8, tray-icon 0.24, raw-window-handle 0.6, x11rb 0.13 (linux), volumectl workspace crate.

**Spec:** `docs/superpowers/specs/2026-10-24-egui-foundation-migration-design.md` — §2/§2.1 feature SSOT, §3 AppCore+EguiSink, §6 parity sweep, §9 cutover gate. Feature SSOT: `docs/references/legacy-app/feature-reference.md`. **Drawn blueprint:** `docs/references/legacy-app/design-spec.md` (tokens, per-screen geometry/states, state machines, parity checklist) — the Task 6 sweep walks it.

---

## SDLC Phase Map

| SDLC phase | Plan location | Exit evidence |
|---|---|---|
| 0. Requirements lock | Spec §2/§2.1 + `feature-reference.md` (user captures authoritative) | Spec approved by user |
| 1. Design lock | Spec §3–§5 (AppCore+EguiSink, Mixer-as-root, glass tokens) | Spec approved by user |
| 2. Implementation | Tasks 1–5 (crate scaffold → surfaces → global behaviors), TDD per task | per-task reviewer gate + ledger |
| 3. System test | Task 6 parity sweep + Task 7 smoke/CLI | sweep doc + benchmark table |
| 4. Release (cutover) | Task 8 | §9 gate evidence printed, records updated |
| 5. Maintenance | post-cutover gap list → new specs | new task rows here or new plan |

## SDD Execution Protocol

- **Workspace/ledger:** `docs/.superpowers/sdd/2026-10-24-egui-foundation-migration/progress.md`; first line `# SDD ledger — plan: docs/superpowers/plans/2026-10-24-egui-foundation-migration.md`. `git clean -fdx` destroys it; recover from `git log`.
- **Per task:** controller runs the SDD `task-start` script (brief + BASE commit), dispatches an **implementer subagent** with the brief, then a **fresh reviewer subagent** on that task's diff. Reviewer gate: spec §2 rows the task claims, test names the brief lists, no hardcoded strings (i18n contract), no `AppCore` API drift, and — for UI tasks — fresh computer-use screenshot + AX evidence attached to the ledger (stale or missing visual proof = task not done).
- **Conflict rule:** spec is binding authority; deviations get `Task N: Ruling: <decision> — <why>` in the ledger before continuing.
- **Task done:** `task-done` script runs the task's full test command; the ledger line `Task N: complete (commits <base>..<head>, tests: <cmd> → <result>)` is written only on green; reviewer findings re-graded by user impact (Critical/Important → one fix pass with RED→GREEN; Minor → ledger as deferred).
- **Final review (after Task 8):** whole-branch package → reviewer on the most capable model; findings fixed in ONE pass; then plan workspace deleted.

## Global Constraints

- MSRV 1.92; edition 2021; workspace resolver 2.
- wgpu pinned at 30.0.1 via egui-wgpu 0.36 (no second wgpu generation).
- `config.ini` remains the SSOT; UI writes only through `AppAction` → `AppCore`.
- **Bilingual via i18n, never hardcoded**: typed `enum Key` + `tr(key, lang, args)` (`{0}` positional interpolation, parity with legacy `translatorFor`); parity test fails the build on missing key or `{n}` placeholder mismatch between EN/VI.
- No silent stubs: a feature impossible on a platform is a recorded degradation.
- **Computer use is mandatory for anything visual**: every task that renders UI (Tasks 1–5, 7) ends its GREEN step with a live computer-use check — launch the surface, screenshot it, exercise the new widget, read the AX tree — before the commit step; screenshot paths + AX findings are ledgered. Code gates alone never close a UI task.
- Every task ends green: task tests + `cargo clippy --workspace --all-targets --no-default-features -- -D warnings` + `cargo fmt --all --check`.
- The old `volumecontrol-host` binary must stay runnable until Task 8.

## Review Focus

1. Hotkey capture works while the Settings viewport is focused (egui per-viewport input).
2. Overlay click-through/always-on-top never leaks onto Settings (per-viewport builder/command wiring).
3. Immediate viewports repaint only while `overlay_animating`; idle = 0 repaints.
4. `SettingsDraft::patch()` mapping byte-identical to the native host's `SettingsPatch`.
5. Session re-poll (1.5 s) runs only while Mixer is visible.
6. Tray actions land on the eframe main thread via channel — no nested event loop.
7. i18n: no literal user-facing strings outside `i18n.rs`; `{n}` parity EN/VI.

---

### Task 1: Scaffold + AppCore/EguiSink + viewport skeleton + vibrancy spike

**Files:**
- Create: `crates/volumecontrol-egui/Cargo.toml`, `src/main.rs`, `src/app.rs`, `src/style.rs`, `src/i18n.rs`, `src/sink.rs`, `tests/scaffold.rs`
- Copy: `crates/volumecontrol-host/src/placement.rs` → `src/placement.rs`; `vibrancy.rs` likewise (rewired per spec §4)
- Modify: root `Cargo.toml` (workspace members), crate deps (eframe 0.36 default features, volumectl, window-vibrancy, raw-window-handle, tray-icon, log, env_logger; UI ecosystem: egui-phosphor 0.14, egui_extras, egui-toast, egui_flex, egui_modal; linux target: x11rb)

**Interfaces:**
- Produces: `VolumeControlApp::new(core: AppCore, opts: LaunchOptions) -> Self`; `LaunchOptions { smoke, smoke_ms, show_all, target_surface }`.
- Produces: `EguiSink: EventSink` — `volume/hotkeys/open_surface/close_surface/toggle_surface/show_tray_menu/exit` → mpsc → eframe loop + `ctx.request_repaint()`; labels "window-mixer"… → `SurfaceId`.
- Produces: `style::apply(ctx, is_dark)` glass tokens; `i18n` typed `Key` + `tr(key, lang, args) -> Cow<'static, str>`.
- Produces: `viewport_id(SurfaceId) -> ViewportId`; `builder_for(SurfaceId, scale) -> ViewportBuilder` (flags per spec §5).

- [ ] **Step 1 (RED):** `tests/scaffold.rs` — (a) i18n: `tr(Key::MixerTitle, Vi, &[])` == "Bộ trộn âm lượng", `tr(Key::UpdateUpToDate, En, &[])` contains "up to date", placeholder parity over all keys; (b) sink: surface-label mapping table; (c) `builder_for` flags per surface (always_on_top/taskbar/transparent/resizable/min-size). Run → compile failure expected.
- [ ] **Step 2:** Implement minimal app: Mixer as root viewport at `frame_for` position, hidden-alive semantics (`CancelClose`+`Visible(false)` on root close), child viewports for Settings/Help/Overlay via `show_viewport_immediate`; vibrancy via title→`FindWindowW`→`Win32WindowHandle` with per-surface applied-flag.
- [ ] **Step 3 (GREEN):** Step 1 tests pass; `cargo run -p volumecontrol-egui -- --surface=mixer` shows styled Mixer (manual, ledgered).
- [ ] **Step 4:** Commit `feat(egui): scaffold eframe host, AppCore sink, i18n, vibrancy via title lookup`
- [ ] **Reviewer gate:** sink channel ordering, no `AppCore` mutation outside `apply_action`, i18n tables verbatim from legacy file.

### Task 2: Mixer — full legacy parity

**Files:** `src/surfaces/mixer.rs`, `tests/mixer.rs`

**Interfaces:** `MixerModel::from(&AppState)` (sort active-first, volume desc); `mixer::show(app, ui)`; actions `SetVolumePercent/ToggleMute/ResetVolume/SetSessionVolume/ToggleSessionMute`; re-poll cadence 1.5 s (Mixer visible only); header actions Show/Hide surfaces.

- [ ] **Step 1 (RED):** `tests/mixer.rs` — sort order; search filter (name/process/display-name, case-insensitive, via typed query); empty/no-match strings through `tr()`; header intents; footer count `{0}` interpolation; muted-state model (status text "Đã tắt tiếng", hidden fill, speaker-off icon variant). Run → fail.
- [ ] **Step 2 (GREEN):** implement model + view: header (title, live %/muted, gear/help/close via Phosphor), search, system row (slider + mute + reset-50), session rows, footer, backend-error/retry notice from `AppState.status`, update banner. Widgets dispatch real `AppAction`s.
- [ ] **Step 3:** Refactor: view ≤ 150 lines, model pure. Commit `feat(egui): mixer surface with full legacy parity`
- [ ] **Reviewer gate:** re-poll gated on visibility (Review Focus 5); all strings via `tr`.

### Task 3: Settings — six tabs, draft, recorders, blacklist, update card

**Files:** `src/surfaces/settings/{mod,model,recorder,blacklist,update_card,views}.rs`, `tests/settings.rs`

**Interfaces:** `SettingsDraft::from_config(&Config)`; `patch() -> SettingsPatch` (mapping identical to native host); `is_pristine()`; `validate() -> Result<(), FieldError{field, msg}>` (field→tab routing); `recorder::RecorderState` (Idle→Recording(action)→Captured; bare key keeps recording; Escape cancels; Backspace clears; portable `Code` format); `blacklist` pure fns (add normalize/dedupe, remove, merge_recommended, replace_defaults, reload_saved, clear); `update_card::Phase` machine (Idle→Checking→Available→Downloading→Ready{confirm}→Installing→UpToDate|Error{retry}|ManagedExternally); picker selection model (dropdown panel under "Chọn ứng dụng đang chạy": checkbox toggle edits draft immediately, "đã chọn: N", **Xong only closes**, reopen preserves draft, Làm mới refreshes).

- [ ] **Step 1 (RED):** `tests/settings.rs` — patch mapping table (every field, both languages where strings); validation cases (overlay <500, step bounds, threshold ordering, beep ranges); recorder transitions incl. bare-key/Escape/Backspace; blacklist fns incl. merge-no-dupes; picker reopen-preserves-draft (no `config.ini` write); update phase transitions. Run → fail.
- [ ] **Step 2 (GREEN):** implement models, then six tab views (Chung incl. update card + autostart; Phím tắt incl. 8 recorders + conflict badges + presets; Giao diện incl. preview + thresholds; Danh sách chặn incl. picker + 4 bulk buttons; Phản hồi full-width numeric rows; Lưu trữ single card + notice). Footer Đặt lại/Huỷ/"Lưu thay đổi" disabled-when-pristine; rejected save keeps draft + inline error routed to owning tab.
- [ ] **Step 3:** Refactor: pure models egui-free. Commit `feat(egui): settings surface with six-tab legacy parity`
- [ ] **Reviewer gate:** Review Focus 4 (patch mapping), 7 (i18n); recorder capture path documented (per-viewport input).

### Task 4: Help + Overlay

**Files:** `src/surfaces/help.rs`, `src/surfaces/overlay.rs`, `tests/help.rs`, `tests/overlay.rs`

**Interfaces:** help rows derive from real config bindings + hotkey status pills; footer `OpenConfigLocation`/`ShowSurface(Settings)`; `OverlayState { last_show, duration: config.overlay_duration_ms, payload }`; e2e-debug hold-open flag; mouse-passthrough only while pure volume toast.

- [ ] **Step 1 (RED):** help rows derive from bindings (8 actions, groups Âm lượng/Lệnh/Mở rộng) + search filter + status pill mapping; overlay timer math (payload arms, debug holds, closes after duration), mute renders "Muted". Run → fail.
- [ ] **Step 2 (GREEN):** implement both surfaces. Commit `feat(egui): help and overlay surfaces`
- [ ] **Reviewer gate:** Review Focus 2, 3.

### Task 5: Global behaviors — tray, single instance, CLI verbs, wheel, autostart

**Files:** `src/surfaces/tray.rs`, `src/cli.rs`, `src/native_win32.rs`, modify `app.rs`, `crates/volumectl/src/linux_host_core.rs` (only where the Linux egui path differs)

**Interfaces:** `TrayMenu` 10 localized items → mpsc → eframe loop; live "Volume Control — {pct}%" header; `cli::run_verb(get|set|mute)`; `native_win32::ensure_single_instance(show_mixer)` port.

- [ ] **Step 1 (RED):** tray item id→action table (10 rows, `tr()`-driven); CLI parse table; single-instance second-launch semantics (ported tests). Run → fail.
- [ ] **Step 2 (GREEN):** implement tray (actions via channel), single-instance + show-mixer event, foreground launch opens Mixer, wheel bridge (Windows), verify blacklist gate + beep already in `AppCore`, wire status into UI. Commit `feat(egui): tray, single instance, cli verbs, wheel volume`
- [ ] **Reviewer gate:** Review Focus 6; tray items text-only via `tr`.

### Task 6: Parity sweep + benchmarks (system test)

**Files:** `docs/references/egui-parity-sweep.md`

- [ ] **Step 1:** computer-use walkthrough of every `feature-reference.md` §1–§5 row on `--show-all`; record pass/fail + screenshot path per row; fix regressions (structural gaps → new tasks).
- [ ] **Step 2:** benchmark table (startup, RSS, idle CPU, drag CPU, 1% low) vs `volumecontrol-host`. Commit `docs(egui): parity sweep results and benchmarks`
- [ ] **Reviewer gate:** sweep doc row count == feature-reference row count; no row silently skipped.

### Task 7: Smoke + CLI contract

**Files:** `main.rs`, `app.rs`

- [ ] **Step 1 (RED):** smoke matrix test — `--smoke` exit 0 with 4 surfaces + toast; `--surface=<id>` opens exactly one; verbs behave. Run → fail.
- [ ] **Step 2 (GREEN):** implement `smoke_seen` across viewports + timer → `Exit` → root `Close`; exit code from `smoke_requested && verified`. Commit `feat(egui): smoke contract and polish`
- [ ] **Reviewer gate:** exit-code path identical to native host semantics.

### Task 8: Cutover (release)

**Files:** delete `crates/liquid_glass/`? **No** — retire old host renderer files (`paint.rs`, old `app.rs`, `sync.rs`); `crates/liquid_glass` stays in workspace untouched (own tests green); keep-or-delete surfaced to user as an explicit decision. Update `feature_list.json`, `claude-progress.md`, `session-handoff.md`.

- [ ] **Step 1:** Print §9 gate evidence (gates 1–5 of the spec) — all green.
- [ ] **Step 2:** Retire old renderer files; workspace builds green.
- [ ] **Step 3:** Records updated with cutover evidence + benchmark table. Commit `refactor!: cut over to egui foundation, retire native scene renderer`
- [ ] **Reviewer gate:** whole-branch final review (SDD), one fix pass, ledger `Final:` lines.

## Risk Register

| Risk | Likelihood | Mitigation | Rollback |
|---|---|---|---|
| Child-viewport vibrancy HWND lookup fails | Med | Task 1 spike; per-surface flag + retry; degradation recorded | Native host remains production |
| Recorder misses chords on some layouts | Med | typed `Code` format + full key-event matrix tests | Fallback: preset modifiers only, recorded |
| Immediate-viewport repaint coupling burns CPU | Med | request_repaint gated on `overlay_animating` | Switch Overlay to deferred viewport (Arc model) |
| egui visual style misses "liquid glass" bar | Med | §6 sweep includes glass screenshots; style.rs tokens iterated | Keep native host until bar met |

## Definition of Done

Spec §9 gates 1–6 all pass with printed evidence; parity sweep doc complete; benchmark table filled; records updated; ledger contains every `Ruling:` and deferred minor, surfaced in the final report.
