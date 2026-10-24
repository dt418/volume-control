# Egui parity sweep — v0.1.11 reference

Walked against `docs/references/legacy-app/feature-reference.md` §1–§5 with
computer use (screenshots + UIA/AX) and the crate tests. `Partial` means the
control exists and dispatches but a reference detail is missing; every partial
is listed in §9 of the design spec and blocks cutover until resolved or
explicitly accepted.

Legend: ✅ pass · ◐ partial · ⛔ gap.

## 1. Global behaviors

| Reference row | Status | Evidence / deviation |
|---|---|---|
| Global hotkeys (8) | ◐ | `AppCore` registers all eight; conflicts are shown per row in Settings/Help. While the legacy `VolumeControl.exe` runs it owns the chords, so the egui host logs “already registered” and receives none. With the legacy app closed the egui host owns them. |
| Custom shortcut recording | ✅ | `settings/recorder.rs` + row UI; bare key keeps recording, Esc cancels, Backspace clears, portable `Code` chords; draft-only until Save. |
| Modifier presets | ✅ | Ctrl+Alt / Alt / Ctrl enabled; Caps Lock disabled with fallback note; preset rewrites the draft and applies on Save. |
| Blacklist gate on hotkeys | ✅ | Implemented in `AppCore::apply_hotkey` (shared reducer); verified by volumectl unit tests. |
| Beep feedback | ✅ | Feedback tab writes `[Beep]` via `SettingsPatch`; `AppCore` beeps blocked/limit paths. |
| Single instance + second launch shows Mixer | ✅ | `single_instance.rs` (separate `Local\VolumeControl.Egui.*` namespace); unit tests cover primary/secondary and show-mixer signal. |
| Autostart | ✅ | General toggle persists `autostart`; Save calls `volumectl::autostart::set_enabled` when changed. |
| Update flow | ◐ | Check (off-thread), UpToDate/Available/Error/Managed phases, localized status. Download/install and the Mixer install banner are not ported; the Available action opens the release page. Recorded degradation, blocks cutover. |
| Config live reload | ✅ | 50 ms core poll calls `reload_config_if_changed`; Storage tab documents INI SSOT. |
| CLI verbs | ✅ | `get` / `set <0-100>` / `mute [on|off]`; verified 100 → 42 → mute → 0. |
| Tray menu (10 items) | ✅ | Dark muda menu, live header, all nine actions wired; polling from the eframe loop. Tray tooltip `Volume Control (egui)` distinguishes it from the legacy tray during migration. Trailing-click automation is limited; `ShowSettings` was observed end-to-end from the tray. |

## 2. Mixer

| Row | Status | Evidence / deviation |
|---|---|---|
| 400×224 borderless, always-on-top | ◐ | Borderless/always-on-top/opaque; height is 240 so one full session card plus a scroll hint is visible (user-requested). Docking math follows the legacy 20/40/16 margins. |
| Header title, live %, gear/help/close | ✅ | Chip header with localized status and icon buttons. |
| Search filter + empty/no-match | ✅ | Case-insensitive over name/process/display name; empty and unsupported states. |
| System row: value, mute, reset, slider | ✅ | Card layout, speaker icon by level, reset-50, SignalRail colors. |
| Session list scroll, System Sounds label/hint | ✅ | Cards, sorted active-first/volume-desc, 1.5 s re-poll gated on visibility, scrollbar when needed. |
| Footer count + Esc | ✅ | Pinned bottom panel; Esc hides the surface. |
| Focus-loss auto-close | ⛔ | Not implemented in the egui host (Esc and close only). Blocks cutover unless accepted. |
| Update-available banner | ⛔ | Waits on the ported update manager. |

## 3. Settings

| Row | Status | Evidence / deviation |
|---|---|---|
| Chrome + 6 tabs + sidebar + footer | ✅ | Opaque shell, localized title/subtitle, sidebar pills, footer Reset/Cancel/Save with pristine state. |
| 3.1 General | ✅ | Language/label style live-relabel, steps, overlay duration ≥500 validation, autostart, update card phases. |
| 3.2 Hotkeys | ✅ | Eight rows, uniform chip/button sizes, recording state machine, conflict badges, presets. |
| 3.3 Appearance | ✅ | Four combos, preview card, threshold editor with ordering validation. |
| 3.4 Blacklist | ◐ | Manual add/remove, suggestions/bulk actions, picker with search/refresh/multi-select and live draft edits. Running-app icons are not rendered (text rows only). |
| 3.5 Feedback | ✅ | Toggle + four numeric rows with range validation. |
| 3.6 Storage | ✅ | Single card, config path, open-config button, recovery notice. |

## 4. Help

| Row | Status | Evidence / deviation |
|---|---|---|
| 520×500, header, search, groups, pills, chips | ✅ | Opaque shell; 8 rows in Âm lượng/Lệnh/Mở rộng; status pills map `HotkeyRegStatus`; conflict callout; footer Edit config/Settings/Close. |

## 5. Overlay HUD

| Row | Status | Evidence / deviation |
|---|---|---|
| 336×88 bottom-right, % + rail + device | ✅ | 20/40 margins; SignalRail band colors; mute uses gray + diamond; text payloads wrap. |
| Auto-hide `overlay_duration_ms` | ✅ | Timer per payload; `--overlay-demo` for deterministic checks. |
| Slide/fade animation | ⛔ | Surfaces appear/disappear instantly; no slide-in tween. |

## Benchmarks (Windows 11, 2560×1440, DX12, debug build)

Measured with `scripts/bench-ui.ps1` (startup = process start → visible main
window; RSS after idle; idle CPU over 10 s).

| Metric | egui host (`volumecontrol-egui --surface=mixer`) | native host (`volumecontrol-host --surface=mixer`) |
|---|---|---|
| Startup → first window | 2,907 ms | 3,373 ms |
| RSS idle | 490 MB | 506 MB |
| Idle CPU (10 s) | 7.19% | 12.03% |
| Slider-drag CPU / 1% low | not measured (scripted drag) | not measured |

## Cutover blockers

1. Update download/install + Mixer install banner.
2. Mixer focus-loss auto-close.
3. Running-app icons in the blacklist picker.
4. Overlay slide animation (or explicit acceptance of the instant show).
5. Hotkey ownership: only one host may run while the legacy app is open; document the migration step or rename the egui chords during coexistence.
