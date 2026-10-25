# Egui Foundation Migration Design (VolumeControl)

**Status:** Draft for user review
**Date:** 2026-10-24
**Supersedes:** native renderer path described in `2026-10-23-rust-ui-liquid-glass-spec.md` §4 (only after the cutover gate below passes)
**Trigger:** user decisions — "Nôi muốn build dự án trên nền egui" (2026-10-24) and scope refinement: **"đầy đủ như bản v0.1.11, chỉ apply giao diện mới có style liquid glass trên cả 3 nền tảng tương thích"** (2026-10-24)

---

## 1. Problem

The production UI is a hand-rolled retained-scene renderer (`crates/liquid_glass`):
manual `Rect` layout, manual glyphon text placement, manual hit-testing, manual
focus traversal, manual AccessKit tree building. Every widget (button, slider,
tab, list) is bespoke code. UI iteration cost is high and visual polish trails
the legacy webview app.

The user has chosen **egui** (immediate-mode, `eframe` 0.36, wgpu 30, first-party
AccessKit) as the new UI foundation, and requires **full feature parity with the
installed v0.1.11 app** — not a subset — with the new liquid-glass visual style
working on Windows, macOS, and Linux (X11; Wayland via compositor rules).

## 2. Feature-parity source of truth

The checklist SSOT is `docs/references/legacy-app/feature-reference.md`
(captured live from the installed v0.1.11 app). Everything in its §1–§5 must
exist in the egui host before cutover:

- **Mixer**: header (title, live % — replaced by "Đã tắt tiếng" status text when muted, Cài đặt/Trợ giúp/Đóng icon buttons), search, system row with mute + reset-50% (muted state: speaker-off icon button, slider fill turns **gray** with an **outlined-diamond thumb** — fill stays visible, per SignalRail), session rows (sort: active first, volume desc; 1.5 s re-poll), footer, update banner, backend-error/retry notices, empty states.
- **Tray (live capture)**: header row "Volume Control — {pct}%", items Tắt tiếng / Đặt lại 50% / Mở bộ trộn âm thanh | Cài đặt / Trợ giúp / Kiểm tra cập nhất / Tải lại cấu hình / Mở file cấu hình | Thoát Điều khiển âm lượng.
- **Settings, 6 tabs**: Chung (language EN/VI, app-label style, numeric volume step + step-large, overlay duration, autostart, update card with full phase machine), Phím tắt (8 shortcut recorders with record/clear, conflict badges, modifier presets Ctrl+Alt/Alt/Ctrl/CapsLock-fallback), Giao diện (theme/material/motion/accent, live preview, volume threshold editor 40/75/100), Danh sách chặn (manual entry + running-app picker with icons, apply-recommended, restore defaults, reload saved, clear all), Phản hồi (beep toggle + 4 fields), Lưu trữ (config path, open config, recovery notices).
- **Help**: 8 shortcuts from real bindings, status pills, search, footer (Sửa cấu hình/Cài đặt/Đóng).
- **Overlay**: threshold-colored rail (single-band fill, gray + diamond thumb when muted), config-driven duration, text toasts; always-on-top, click-through while a pure volume toast is shown (interactive for payload toasts).
- **Tray**: 10-item menu with live volume label, localized.
- **Global**: single instance + second-launch shows Mixer, autostart migration, background update checks/install, **native update notifications (vol-116 Windows toast; fallback in-app banner, recorded)**, blacklist gate on hotkeys, beep feedback (blocked/limit), mouse-wheel volume, CLI verbs (`get`/`set`/`mute`), Mixer opens on foreground launch.

### 2.1 Settings tab contracts (detailed)

**Chung**: Ngôn ngữ ComboBox (Tiếng Việt/English — live relabel); Tên ứng dụng hiển thị ComboBox (Tên thân thiện/Tên tiến trình); Bước âm lượng numeric field 1–50; Bước lớn 1–50 (Shift+arrows); Thời gian hiện overlay ≥500 ms (reject lower with inline error); Khởi động cùng Windows toggle (commit qua Save, availability status from host); **Update card** phases: `Idle(version + "Kiểm tra cập nhật") → Checking → Available{version, "Tải xuống"} → Downloading{pct} → Ready{confirm dialog → install_update + restart} → UpToDate("Điều khiển âm lượng đã là bản mới nhất") | Error{retry} | ManagedExternally` (winget-managed builds show managed note, no install button).

**Phím tắt**: 8 rows (Tăng/Giảm, Tăng/Giảm lớn, Bật-tắt tiếng, Đặt lại 50%, Mở bộ trộn, Mở menu); each row `[mô tả] [chip binding — click để ghi đè] [Ghi] [Xoá]`; recording state machine `Idle→Recording(action)→Captured`, bare key keeps recording with hint, Escape cancels, Backspace clears, portable `Code` format; conflict → red badge "hotkey already registered by another application"; presets Ctrl+Alt / Alt / Ctrl / CapsLock (disabled, note "Caps Lock dùng dự phòng Ctrl + Alt"); preset click rewrites all 8 bindings in draft.

**Giao diện**: Chủ đề/Chất liệu/Chuyển động/Màu nhấn ComboBox; live Preview card (mini slider tinted by draft material/threshold tokens); Ngưỡng âm lượng 3 numeric fields (Xanh lá tới 40 / Xanh dương tới 75 / Cam tới 100) with ordering validation.

**Danh sách chặn**: text entry "vd: chrome.exe" + Thêm (trim/lowercase/dedupe); rows with per-row Xoá; **Chọn ứng dụng đang chạy** opens a **dropdown picker panel** anchored under the button: search field + "Làm mới" refresh, running-app rows with **checkbox multi-select** (icon + friendly name), counter "đã chọn: N", close button "Xong". Semantics (per `RunningAppPicker.tsx`): **checkbox toggles edit the blacklist draft immediately**; "Xong" only closes the panel; persistence happens solely via footer "Lưu thay đổi". Áp dụng gợi ý (merge presets, no dupes), Khôi phục mặc định (replace), Tải lại đã lưu (discards draft, disabled when list pristine), Xoá hết; empty state "Chưa chặn ứng dụng nào / Điều khiển âm lượng sẽ phản hồi phím tắt ở mọi nơi."; note "Thay đổi chỉ áp dụng khi bạn lưu."

**Phản hồi**: "Bật phản hồi âm bíp" toggle + 4 numeric fields (blocked freq 400 Hz, blocked duration 80 ms, limit freq 600 Hz, limit duration 60 ms) with range validation. Numeric inputs render as **full-width stacked rows**, not inline spinners.

**Lưu trữ**: content sits in a **single card** (config path box + "Mở config.ini (nâng cao)" button + note together); recovery notice from bootstrap (`config_notice`) renders as a warning row inside that card.

**Shared chrome (every tab)**: window header "Cài đặt Điều khiển âm lượng" + subtitle "Cấu hình âm lượng, phím tắt, giao diện và phản hồi" + Đóng button stays visible across tab switches; footer buttons are Đặt lại (disabled-when-pristine), Huỷ, and **"Lưu thay đổi"** (accent, disabled-when-pristine).

## 3. Reducer choice — AppCore + EguiSink

- The egui host binds to **`volumectl::host_core::AppCore`** — the full
  production reducer the legacy app uses (blacklist gate, beep feedback,
  session merge, update manager, config persistence are implemented there).
  Constructor: `AppCore::new(audio: Box<dyn AudioBackend>, config: Config,
  modifier: HotkeyModifier, sink: Arc<dyn EventSink>)`.
- The host implements **`EguiSink: EventSink`** as the bridge:
  `volume(pct, muted)` / `hotkeys(&[HotkeyRegResult])` / `open_surface(label)` /
  `close_surface(label)` / `toggle_surface(label)` / `show_tray_menu()` /
  `exit()` arrive on the core's thread and are forwarded over an mpsc channel
  into the eframe loop (state mirror + `ctx.request_repaint()`); surface labels
  ("window-mixer"…) map to `SurfaceId`.
- Every user intent flows back as `AppAction` via `core.apply_action(&action)`
  — identical to the native host's wiring.
- `linux_host_core.rs::HostCore` (the simplified Linux-GTK host reducer with
  stub arms) is **not** used by the egui host; platform-specific gaps instead
  degrade gracefully in the egui sink/CLI layer and are recorded, never
  silently stubbed.

## 4. Verified platform facts (researched 2026-10-24, egui/eframe 0.36.2)

- `eframe` ships `accesskit` as a default feature — first-party accessibility, no manual tree building.
- `egui-wgpu` 0.36 targets wgpu 30 — matches the workspace pin (30.0.1); no duplicate dependency generation.
- Multi-window: `Context::show_viewport_immediate` / `show_viewport_deferred`; immediate viewports take `&mut self` state directly (chosen for this codebase); deferred needs `Arc` + locks.
- `ViewportBuilder` covers our window needs: `with_transparent`, `with_decorations(false)`, `with_always_on_top`, `with_window_level`, `with_taskbar(false)` (Windows), `with_mouse_passthrough`, `with_position`, `with_inner_size`, `with_min_inner_size`, `with_resizable`, `with_visible`, `with_active`.
- `ViewportCommand` covers runtime diffing: `Close`, `Visible`, `OuterPosition`, `InnerSize`, `WindowLevel`, `MousePassthrough`, `IMEAllowed`, `Focus`.
- eframe 0.36 `App` trait: `fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame)` (root panel `Ui` is passed in; no `update(ctx, frame)`).
- Per-child-viewport vibrancy: `ViewportInfo` exposes **no native window field**, so `window-vibrancy`'s `HasWindowHandle` path is unreachable from egui viewports. Resolved mechanism — **PID-scoped, not title-unique**: `FindWindowW` by title alone is ambiguous because the preserved native host uses the same titles; instead `EnumWindows` + `GetWindowThreadProcessId` filtered to the **current process id**, then title match → HWND → wrap in a `Win32WindowHandle` implementing `HasWindowHandle` → `apply_mica`/`apply_acrylic`. Cache keyed by **live HWND** (re-validate each frame; a dead HWND re-triggers lookup), and effects re-apply after a viewport is destroyed/reopened — the applied state is per-HWND, never a permanent surface boolean. Requires `windows-sys` (`Win32_UI_WindowsAndMessaging` already in workspace features).

## 5. Architecture

Root viewport **is the Mixer surface** (always-on-top, no taskbar, 400×224 at
`frame_for` position). `close_requested` on the root is answered with
`ViewportCommand::CancelClose` + `Visible(false)` (mirrored in a `root_shown`
flag), so closing the Mixer leaves the process resident — legacy behavior.
Settings, Help, and Overlay are `show_viewport_immediate` child viewports,
called every frame while `AppState.is_visible(surface)`; stopping the call
closes the window. Esc inside any viewport → `HideSurface`.

```text
crates/volumecontrol-egui (new binary, eframe host)
  ├─ main.rs          CLI parity: --smoke [--smoke-ms=N] | --show-all | --surface=<id>
  │                   + legacy CLI verbs: get | set <pct> | mute [on|off]
  ├─ app.rs           VolumeControlApp (eframe::App): pump loop, viewport diffing, smoke,
  │                   single-instance + second-launch mixer request, tray integration
  ├─ style.rs         Liquid Glass design tokens for egui visuals (translucent panels,
  │                   rounded corners, accent, spacing) — one system, 3 platforms
  ├─ surfaces/
  │    ├─ mixer.rs    full legacy parity incl. header, notices, sort, re-poll, banner
  │    ├─ settings.rs 6 tabs incl. hotkey recorders, blacklist editor, thresholds,
  │    │              feedback, storage, update card
  │    ├─ help.rs     8 shortcuts from real bindings + status pills + footer
  │    ├─ overlay.rs  HUD card, threshold rail, auto-hide timer, text payload
  │    └─ tray.rs     tray-icon menu (native tray primitives, localized)
  ├─ placement.rs     reused from old host (frame_for / logical_size)
  └─ vibrancy.rs      reused; per-viewport application (see §4 open point)

kept untouched:
  crates/volumectl          HostCore (full reducer), audio, config.ini, hotkeys
  window-vibrancy           OS backdrop (Mica/Acrylic/NSVisualEffectView)
  global hotkeys            Ctrl+Alt+↑/↓/M/V/R … + custom bindings
```

Surfaces are immediate viewports so all state is `&mut self`; the only
animation (Overlay fade/threshold rail) drives `ctx.request_repaint()` for its
duration, so idle CPU stays 0.

> **Task 1 spike (hidden-root painting):** verify that immediate child viewports keep painting while the Mixer root is `Visible(false)`; if children stall, fall back to a 1×1 transparent off-screen root. Recorded in the ledger before Task 2.

## 6. Acceptance: parity sweep against the legacy reference

The cutover checklist is a **walkthrough of every row of
`docs/references/legacy-app/feature-reference.md` §1–§5** performed with
computer use on the egui binary: each control must render, dispatch its real
action (volume actually changes, `config.ini` actually persists on Save,
blacklist actually gates a hotkey), and the UIA tree must announce full names.
Row-by-row results are recorded in the session log; any unported row blocks
cutover.

Platform notes for the liquid glass style:
- Windows 11: Mica (Settings) / Acrylic (HUDs) via window-vibrancy, premultiplied transparency.
- macOS: `NSVisualEffectView` per viewport (HudWindow/Sidebar materials).
- Linux X11: `_KDE_NET_WM_BLUR_BEHIND_REGION` atom (KWin/Picom); Wayland: compositor window rules, documented.
- egui visuals themselves (translucent panel fills, rounded corners, accent) come from one `style.rs` token set shared by all platforms — the "glass" reads even without compositor blur.

A11y acceptance: egui's built-in AccessKit tree exposes slider values, button labels, tab semantics, recorder states — verified by computer-use UIA read; labels announce full names, never display-truncated ones.

Visual contract for the glass look (all platforms, one `style.rs`):
- Window/panel fills translucent dark (`Color32` alpha ≈ 180) so vibrancy backdrop shows through; `eframe::App::clear_color()` transparent because `with_transparent(true)`.
- **Light theme parity**: legacy supports ThemeMode System/Sáng/Tối — `style.rs` carries a light token set (translucent light fills, dark text) resolved from `AppState.theme` like the legacy appearance applier.
- Widget rounding 8 px, spacing 8/16 grid, accent `#00a4ef` family for selected/active states; Phosphor icons for mute/reset/close/gear/search.
- **Persistence policy**: eframe's own storage is DISABLED (`persistence` off / no `app.ron`) — `config.ini` stays the only SSOT; window placement comes from `placement::frame_for`, not saved egui state.

Copy contract — **no hardcoded user-facing strings**:
- The legacy app is bilingual via `frontend/src/lib/i18n.tsx` (~250 EN/VI keys). That key set is the SSOT: port it verbatim into `crates/volumecontrol-egui/src/i18n.rs`; `lang` comes from `Config.language` already published in `AppState`.
- Signature: **typed keys** (`enum Key`) + `tr(key, lang, args: &[&str]) -> Cow<'static, str>` — the legacy translator replaces positional `{0}`, `{1}`… placeholders (`translatorFor`, i18n.tsx:481-487), which strings like update-available/ready/error and session counts depend on; `args` substitutes them.
- Every label, placeholder, tooltip, notice, tray item, and validation message renders through `tr()` — Mixer, Settings, Help, Overlay, tray menu, CLI output.
- Parity unit tests: every `Key` present in both EN and VI maps (missing key = build failure); **placeholder parity** — the `{n}` set of a key must match between EN and VI; formatted tests for version/count/error interpolations. Keys not present in the legacy file may not be invented ad hoc — extend the legacy file first.

## 7. Benchmark contract (same as Dioxus prototype protocol)

Baseline: `crates/volumecontrol-host` Settings surface on this machine.
| Metric | Method |
|---|---|
| Startup → first visible Settings frame | process launch → visible-window timestamps |
| RSS idle (10 s) | Windows process working set |
| CPU idle (30 s) | process CPU-time delta |
| Slider-drag CPU (60 s) | CPU-time delta + frame intervals |
| 1% low frame time | repaint timestamps under repeated interaction |
| AccessKit parity | computer-use AX tree comparison |
| Glass look | screenshots vs native (vibrancy + egui translucent visuals) |

## 8. Out of scope

- Custom WGSL SDF glass per widget (Liquid Glass via `PaintCallback`) — possible follow-up; this migration ships egui translucent visuals + OS vibrancy as the glass style.
- Nothing from the v0.1.11 feature set is out of scope (that is the point of §6); any row that proves impossible on a platform is a recorded, user-visible degradation — not a silent drop.

## 9. Cutover gate (all must pass)

1. `cargo test --workspace --no-default-features` green (new egui tests + remaining suites).
2. `cargo clippy --workspace --all-targets --no-default-features -- -D warnings` clean; `cargo fmt --all --check` clean.
3. `--smoke` exits 0 with all four surfaces presented and the toast shown; CLI verbs (`get`/`set`/`mute`) behave like the legacy binary.
4. §6 parity sweep: every feature-reference row passes on the egui binary.
5. Benchmark table filled with measured numbers on this machine.
6. Only then: retire the old host renderer (`paint.rs`, old `app.rs`, `sync.rs`); `crates/liquid_glass` stays in the workspace untouched (its own tests keep it green) and its keep-or-delete status is surfaced to the user as an explicit decision, never deleted unilaterally. Records updated.

If any gate fails, the native host remains production; the egui crate stays with a written gap list.
