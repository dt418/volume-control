# Legacy App Feature Reference (installed build v0.1.11)

**Purpose:** complete, evidence-based inventory of the production Tauri/React app as
installed and running on the reference machine (PID `VolumeControl`, v0.1.11, Windows).
Captured 2026-10-24 via computer use (UI Automation trees + screenshots) against the
running app, cross-referenced with source (`frontend/src/**`, `src-tauri/src/**`) and
the design spec (`docs/superpowers/specs/2026-10-23-rust-ui-liquid-glass-spec.md`).
This is the parity target for the native renderer (`crates/liquid_glass` +
`crates/volumecontrol-host`).

Screenshots live in [`img/`](img/) (per-surface, per-tab, top + scrolled-bottom states).

**Live captures supplied by the user (2026-10-24, authoritative):**
`tray-menu-live.png` (tray menu with live "Volume Control — 100%" header), `mixer-muted.png` (muted state: "Đã tắt tiếng" status text beside title, speaker-off icon, hidden slider fill), `settings-appearance-dropdown.png` (theme ComboBox open: Hệ thống/Sáng/Tối), `settings-blacklist-picker.png` (inline picker panel with checkbox multi-select, "đã chọn: 0", Xong, Làm mới), `update-checking.png` ("Đang kiểm tra cập nhật…" checking state), `settings-feedback.png`, `settings-storage.png`/`settings-storage-full.png`, plus three additional tab captures (`settings-tab-x1..x3.png`). These override any prose where they differ.

---

## 1. Global behaviors

| Behavior | Evidence (live / source) | Detail |
|---|---|---|
| Global hotkeys (8) | Help surface; `HotkeysSection.tsx` | Up `Ctrl+Alt+↑`, Down `Ctrl+Alt+↓`, Up-large `Ctrl+Alt+Shift+↑`, Down-large `Ctrl+Alt+Shift+↓`, Mute `Ctrl+Alt+M`, Reset-50 `Ctrl+Alt+R`, Open Mixer `Ctrl+Alt+V`, Open Menu `Ctrl+Alt+Shift+M` |
| Custom shortcut recording | Settings → Phím tắt | Per-action `Ghi` (record next chord) / `Xoá` (clear); changes draft-only until Save |
| Modifier presets | Settings → Phím tắt → Bộ đặt sẵn | `Ctrl+Alt` / `Alt` / `Ctrl` / `Caps Lock` (disabled = CapsLock is a fallback alias, never a primary) |
| Blacklist gate on hotkeys | `host_core.rs:429-444` | Hotkeys ignored while a blacklisted app has focus |
| Beep feedback | Settings → Phản hồi | Toggle + blocked beep (400 Hz / 80 ms) + limit beep (600 Hz / 60 ms) |
| Single instance + second-launch shows Mixer | `native_win32.rs:77` | `CreateMutexW`; foreground relaunch asks running host to reveal Mixer |
| Autostart | Settings → Chung | Registry run entry; `--background` flag on login |
| Update flow | Settings → Chung → Cập nhật | Current version label, "Kiểm tra cập nhật", status line ("Điều khiển âm lượng đã là bản mới nhất"); Mixer banner offers install; download + restart |
| Config live reload | slow poll | Edits to `config.ini` auto-reload (Storage tab documents this) |
| CLI verbs | `src-tauri/src/main.rs:26-64` | `get` / `set` / `mute` volume from terminal |
| Tray menu (10 items) | `tauri_tray.rs:46-135` | Volume label row, Mute ✓, Reset 50%, Mixer, Settings, Help, Check updates, Reload config, Edit config, Exit — localized, live-relabelled. (Live menu popup capture was not achievable via automation; structure from source.) |

---

## 2. Mixer (`mixer-live.png`) — 400×224, borderless, always-on-top

Layout top→bottom (from live UIA tree + screenshot):

1. **Header**: title "Bộ trộn âm lượng" · live "Âm lượng {n}%" · buttons **Cài đặt** (gear → opens Settings), **Trợ giúp** (? → opens Help), **Đóng bộ trộn** (X)
2. **Search field**: placeholder "Tìm ứng dụng…" — case-insensitive filter over name/process/display-name
3. **System output row** "Đầu ra hệ thống": live `{n}%` label · mute speaker button ("Tắt tiếng đầu ra hệ thống") · **reset-to-50%** button ("Đặt âm lượng về 50%") · slider (0–100)
4. **Session list** (bounded scroll): per app — friendly name (+ System Sounds special-cased: "Âm thanh hệ thống" + subtitle "Thông báo và cảnh báo") · slider · mute speaker button with descriptive aria-label
5. **Footer**: "{n} phiên ứng dụng" · "Esc để đóng"

Interactions: slider drag (optimistic + echo-jitter guard), mute toggle with fail-revert notice, search filter with empty/no-match states, Esc closes, focus-loss auto-closes (after first focus), update banner when update available.

Sort: active sessions first, then by volume descending (`sessionStore.ts:66-70`); session ids re-polled every 1.5 s so stale instance ids are never written.

---

## 3. Settings (`settings-*.png`) — 760×620 (min 620×520), resizable, decorated

Header: "Cài đặt Điều khiển âm lượng" + "Cấu hình âm lượng, phím tắt, giao diện và phản hồi" + Đóng.
Sidebar: **Chung · Phím tắt · Giao diện · Danh sách chặn · Phản hồi · Lưu trữ** (6 sections).
Footer: **Đặt lại** (reset draft to defaults, disabled when pristine) · **Huỷ** · **Lưu thay đổi** (disabled until draft differs; validation errors route to the offending section).

### 3.1 Chung (`settings-general.png`, `settings-general-bottom.png`)
| Control | Default | Notes |
|---|---|---|
| Ngôn ngữ | Tiếng Việt | combobox EN/VI — live relabel |
| Tên ứng dụng hiển thị | Tên thân thiện | combobox: friendly name vs exe name (mixer + blacklist labels) |
| Bước âm lượng | 1 | spinner 1–50 (↑/↓ step) |
| Bước âm lượng lớn | 10 | spinner (Shift+↑/↓ step) |
| Thời gian hiện overlay | 1800 | ms, min-validated (e2e asserts rejection of 100) |
| Khởi động cùng Windows | toggle | registry autostart |
| Cập nhật | v0.1.11 | version label + "Kiểm tra cập nhật" + status line (UpdateCard phase machine: check → available → download % → install + restart / managed-externally / error-retry) |

### 3.2 Phím tắt (`settings-hotkeys.png`, `settings-hotkeys-bottom.png`)
Bàn phím intro ("Nhấn Record rồi nhấn phím bổ trợ cùng một phím…"). 8 rows, each:
`[label + description] [shortcut chip, click to rebind] [Ghi] [Xoá]` — conflicts show badges;
presets row (Ctrl+Alt / Alt / Ctrl / Caps Lock-disabled with fallback note).

### 3.3 Giao diện (`settings-appearance.png`, `settings-appearance-bottom.png`)
Comboboxes: Chủ đề (Hệ thống) · Chất liệu (Tự động) · Chuyển động (Đầy đủ) · Màu nhấn (Hệ thống).
Live **Preview** card — "preview tracks the draft; only Save applies".
**Ngưỡng âm lượng** (slider color thresholds): Xanh lá tới 40 · Xanh dương tới 75 · Cam tới 100.

### 3.4 Danh sách chặn (`settings-blacklist.png`, `settings-blacklist-bottom.png`)
Text entry ("vd: chrome.exe") + **Thêm** · **Chọn ứng dụng đang chạy** (picker with search, refresh, icons) ·
empty state "Chưa chặn ứng dụng nào / Điều khiển âm lượng sẽ phản hồi phím tắt ở mọi nơi." ·
**Áp dụng gợi ý** (adds recommended) · **Khôi phục mặc định** (replaces list) · **Tải lại đã lưu** (disabled without entry, discards edits) · **Xoá hết** · note "Thay đổi chỉ áp dụng khi bạn lưu."

### 3.5 Phản hồi (`settings-feedback.png`, `settings-feedback-bottom.png`)
Checkbox "Bật phản hồi âm bíp" (ON by default) + 4 spinners: blocked freq 400 Hz, blocked duration 80 ms, limit freq 600 Hz, limit duration 60 ms.

### 3.6 Lưu trữ (`settings-storage.png`, `settings-storage-bottom.png`)
Explains the window is the SSOT; path shown: `C:\Users\Thanh\AppData\Roaming\volume-control\config.ini`;
button **Mở config.ini (nâng cao)**; note: direct edits auto-reload, `config.json` kept only as legacy backup.

---

## 4. Help (`help-live.png`) — 520×500 dialog

Header: "Điều khiển âm lượng" / "Phím tắt bàn phím" + Đóng.
**Search field** "Tìm phím tắt…" filters rows live.
Groups: **Âm lượng** (Tăng/Giảm), **Lệnh** (Bật/tắt tiếng, Mở bộ trộn, Đặt lại 50%), **Mở rộng** (Tăng lớn, Giảm lớn, Mở menu) — 8 rows total, each with status pill ("Sẵn sàng" / fallback / in-use / disabled — reflects `state://hotkeys`) and key chips.
Conflict callout when a chord is taken. Footer: **Sửa cấu hình** (opens config.ini) · **Cài đặt** (opens Settings) · **Đóng**.

## 5. Overlay HUD (`overlay-toast.png`) — 336×88, bottom-right

Toast captured live: "Âm lượng 51%" + threshold-colored rail + "Đầu ra hệ thống". Slides in on any
volume change; supports text payload (update/config-reload messages). Auto-hide duration is
configurable (`overlay_duration_ms`, live-configured 1800 ms); the dismissal timing itself is
**test-only evidence** (Vitest fake-timer suite) — e2e explicitly disables it.

---

## 6. Test coverage mapping

### 6.1 e2e (WDIO, `e2e/tauri/specs/`)

| Spec | Behaviors pinned | Platform note |
|---|---|---|
| `mixer.e2e.ts` | bounded shell + accessible System Output controls; search filter + empty state; mute round-trip; update-state read | all platforms |
| `settings.e2e.ts` | all 6 sections; Save disabled on clean draft; draft-only edits + Reset; validation feedback (overlay duration 100 rejected); shortcut recording in draft; update card | all platforms |
| `help.e2e.ts` | shortcut cards + status badges + footer; search filter; Settings-from-Help open (no host teardown) | all platforms |
| `overlay.e2e.ts` | HUD card + rail render after `adjust_volume`, live `%` text, no runtime errors. **Does NOT prove auto-hide** — the debug E2E marker disables the auto-hide timers (host + frontend), and this spec runs **macOS/Linux only** (Windows legacy routes overlay to the native Win32 HUD) | macOS/Linux CI only |
| `recovery.e2e.ts` | bootstrap-failure alert keeps shell mounted | all platforms |
| `windows.e2e.ts` | owned-window enumeration + bootstrap→ready timing | Windows |
| `runtime.e2e.ts` | global API + WDIO guest bridge | all platforms |

### 6.2 Frontend unit tests (Vitest) — behavior assertions the e2e suite does not cover

| Suite | Pinned behaviors |
|---|---|
| `OverlaySurface.test.tsx` | bootstrap render on mount; live update from `state://volume`; text-payload card from `state://overlay`; Muted rendering; **auto-hide timer suite (fake timers): timer not armed without payload, held open under `e2e_debug`, window closed after the configured duration once a payload arrives** (this is where auto-hide is actually proven) |
| `SettingsSurface.test.tsx` | six-section nav; bootstrap failure alert; draft-only edits; Save commits `update_settings` patch + `set_modifier`; rejected save retains edits + inline field error routed to the owning section (incl. beep-field routing to Phản hồi); Reset restores committed state; Cancel closes surface; Saved status disables Save; modifier picker with CapsLock disabled + fallback note; red conflict badges, live-updated from `state://hotkeys` |
| `ShortcutRecorder.test.tsx` | records modifier+key in portable `Code` format; bare key keeps recording with explanation; Escape cancels; Backspace clears |
| `BlacklistEditor.test.tsx` | legacy empty-state copy; normalize (` Code.exe ` → `code.exe`) + duplicate rejection; remove/clear; Apply Recommended merges without duplicates; Restore defaults replaces; Reload saved discards draft; running-app picker adds an app; process-style keeps exact names |
| `UpdateCard.test.tsx` | version label pre-check; check button → `check_for_updates`; available/downloading states; **ready state with confirmation dialog + `install_update`**; up-to-date status; error state + retry |
| `VolumeThresholdEditor.test.tsx` | three threshold inputs with draft values; draft edits; inline backend-field errors |
| `FeedbackSection.test.tsx` | beep switch + four legacy-labelled inputs; draft edits; inline error for rejected field |
| `StorageSection.test.tsx` | config path from `config_path`; open button → `open_config_location`; auto-reload note; INI-canonical + legacy JSON backup copy; bootstrap `config_notice` recovery warning |

### 6.3 Evidence classification used in this document

- **verified-live** (computer use on the installed app, this machine): Mixer layout/controls/header buttons, all 6 Settings tabs incl. scrolled bottoms, Help groups + footer, Overlay toast, hotkey open/close of surfaces, config path display.
- **source-only** (read from repo code, not exercised live): tray menu structure, blacklist/beep enforcement points, single-instance, wheel volume, CLI verbs, update install flow internals.
- **test-only** (asserted in Vitest/WDIO, not run in this session): auto-hide timer math, shortcut-record chord capture, conflict-badge live updates, update phases, picker behaviors.

---

## 7. Parity gaps: native renderer vs this reference

Condensed from the full audit (source-level tables in session history). Status: ✅ present · 🟡 partial · ❌ missing.

**Mixer** — ✅ session sliders/mute, master mute, search, Esc. 🟡 system-sounds label (hardcoded EN, no hint line), no clear-search button. ❌ header buttons (Cài đặt/Trợ giúp/Đóng), live % label, reset-50% button, session sort, threshold color bands + muted diamond, backend-error/retry notices, optimistic drag guard, 1.5 s session re-poll, update banner, focus-loss close, empty states, app-label-style names.

**Settings** — ✅ draft save/cancel/reset, language, theme/material/motion/accent. 🟡 autostart (no status read), step buttons (fixed 1/2/5 vs numeric), preview card (label only), beep toggle (not persisted). ❌ Phản hồi + Lưu trữ tabs, app-label style, numeric step + step-large fields, overlay duration, hotkey recorders + presets + conflict badges, threshold editor, blacklist editor + app picker, update card phases, inline validation + section routing, status lines.

**Help** — ✅ 5 static bilingual rows, close. ❌ search, config-driven shortcut list (8 actions, real bindings), status pills, conflict callout, CapsLock note, footer Sửa cấu hình/Cài đặt.

**Overlay** — ✅ HUD %, springs, toasts. ❌ threshold-colored rail, config-driven auto-hide (hardcoded 1500/2500 ms), live refresh while visible.

**Tray / Global** — ❌ tray + menu, single instance, second-launch mixer, wheel volume, blacklist gate, beep on hotkey events, background update checks/install, CLI verbs, config-notice plumbing, mixer-on-launch. ✅ hotkeys (preset modifiers only), config mtime reload, AccessKit.

**Stubbed `AppAction`s in native host** (`linux_host_core.rs:313-329`): `OpenTrayMenu`, `OpenConfigLocation`, `ApplyConfig`, `CancelConfig`, `ResetConfig`, `AddBlacklistEntry`, `RemoveBlacklistEntry`, `ClearBlacklist`, `ApplyRecommendedBlacklist`. `SetTheme/SetMaterial/SetMotion/SetAccent/SetLanguage` mutate in-memory only (no persistence outside `ApplyConfigDraft`).

### Design-spec cross-check (D3 spec)
§4.2 requires the draft contract (implemented); §4.4 Help card requires "application version and
documentation links" (missing); §4.5 requires the tray menu (missing — no OS tray in
`volumecontrol-host`); §7 scenarios 1-3 (HUD latency, per-session mute isolation, draft
isolation) are partially proven by current smoke/computer-use evidence.
