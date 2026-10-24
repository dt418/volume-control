# VolumeControl egui Design Tokens

Single token source: `crates/volumecontrol-egui/src/style.rs`. Every surface
must use these tokens; no ad-hoc hex values in views.

## 1. Color primitives

Neutral ramp (dark → light): `N0 #0B0C0E`, `N1 #17181B`, `N2 #1E2023`,
`N3 #232529`, `N4 #2E3134`, `N5 #3A3D42`, `N6 #5F6670`, `N7 #9AA0A6`,
`N8 #C7CBD1`, `N9 #E8EAED`, `N10 #FFFFFF`.

Accent ramp: `A0 #1B5FC4`, `A1 #2F7FF7`, `A2 #3B96FF`, `A3 #7FBCFF`.

Semantic: success `#22C55E`, warning `#F59E0B`, danger `#EF4444`,
info `#38BDF8`.

## 2. Semantic tokens

| Token (code) | Dark | Light | Usage |
|---|---|---|---|
| `background` | `N1 @ 92%` | `#F5F6F8 @ 96%` | window fill |
| `card` | `N2 @ 91%` | `#FFFFFF @ 94%` | cards, list rows |
| `input` | `N3 @ 94%` | `#EDEFF2 @ 96%` | inputs, search pill, hover fill |
| `border` | `N4` | `#D8DCE1` | 1 px card/row borders |
| `border_strong` | `N5` | `#B9C0C8` | focused input border |
| `text` | `N9` | `#1B1D20` | primary copy |
| `muted` | `N7` | `#5F6670` | hints, secondary copy |
| `text_disabled` | `N6 @ 60%` | `N6 @ 50%` | disabled labels |
| `accent` | `A1` | `A1` | primary buttons, active tab, slider band 2 |
| `accent_hover` | `A2` | `A2` | hover on accent controls |
| `success` | `#22C55E` | `#16A34A` | slider band 1, ready pills |
| `warning` | `#F59E0B` | `#B45309` | slider band 3, recovery notices |
| `danger` | `#EF4444` | `#DC2626` | errors, destructive actions |
| `focus_ring` | `A2 @ 70%` | `A2 @ 60%` | keyboard focus outline |

Alpha values keep the OS backdrop (Mica/Acrylic/NSVisualEffectView) visible
through the surface while text stays readable.

## 3. Spacing scale

| Token | Value | Usage |
|---|---|---|
| `SPACE_1` | 2 px | hairline gaps |
| `SPACE_2` | 4 px | inline icon gap |
| `SPACE_3` | 6 px | intra-card row gap |
| `SPACE_4` | 8 px | default item spacing |
| `SPACE_5` | 12 px | card inner padding (compact) |
| `SPACE_6` | 16 px | card padding, section gap |
| `SPACE_7` | 24 px | large section gap |
| `SPACE_8` | 32 px | window-level gutter |

## 4. Radius scale

| Token | Value | Usage |
|---|---|---|
| `RADIUS_SM` | 4 px | pills, chips, thumb hit area |
| `RADIUS_CONTROL` | 8 px | buttons, inputs, cards in lists |
| `RADIUS_CARD` | 12 px | settings content card |
| `RADIUS_HUD` | 16 px | mixer/overlay shell |

## 5. Typography scale

Base size = `Config.ui_font_size` (9–24, default 15). Every text style derives
from it; views must not hardcode `size(...)`.

| Token | Multiplier | At 14 px | Usage |
|---|---|---|---|
| `caption` | base − 4 | 10 | footer hints, tiny labels |
| `small` | base − 2 | 12 | hints, status, chips |
| `body` | base | 14 | labels, buttons, inputs |
| `title` | base + 4 | 18 | surface titles |
| `headline` | base + 8 | 22 | overlay percentage |
| `display` | base + 14 | 28 | empty-state numerals |

Weights: regular for body/hints, semibold (`.strong()`) for titles, buttons,
and active values.

## 6. Control sizes

| Token | Value | Usage |
|---|---|---|
| `ICON_BUTTON` | 24×24 | header gear/help/close, mute/reset |
| `BUTTON_HEIGHT` | 28 | footer buttons, recorder chips |
| `INPUT_HEIGHT` | 30 | search, combos |
| `ROW_HEIGHT` | 32 | settings rows, hotkey rows |
| `SLIDER_HEIGHT` | 18 (track 6) | mixer/HUD rails |

## 7. Motion

| Token | Value | Usage |
|---|---|---|
| `DURATION_FAST` | 120 ms | hover/press feedback |
| `DURATION_NORMAL` | 180 ms | overlay fade, tab switch |
| `DURATION_SLOW` | 240 ms | surface open/close |
| `EASING_STANDARD` | ease-out cubic | all transitions |

Reduced-motion mode (`MotionMode::Reduced/Disabled`) clamps these to 0.

## 8. Elevation and layering

| Layer | Token | Usage |
|---|---|---|
| background | `background` | window shell |
| raised | `card` + 1 px `border` | cards, rows |
| overlay | `card` + border + `RADIUS_HUD` | HUD, toasts |
| scrim | `#000 @ 40%` | modal/confirm backdrop |

No drop shadows: separation comes from fill, border, and spacing.

## 9. Surface geometry

Declared once in `crates/volumecontrol-egui/src/style.rs` and re-exported by
`placement.rs`; `builder_for` and every surface view consume these constants.

| Constant | Value | Usage |
|---|---|---|
| `MIXER_WIDTH` / `MIXER_HEIGHT` | 400 × 240 | Mixer root viewport |
| `SETTINGS_DEFAULT_WIDTH` / `SETTINGS_DEFAULT_HEIGHT` | 760 × 620 | Settings window |
| `SETTINGS_MIN_WIDTH` / `SETTINGS_MIN_HEIGHT` | 620 × 520 | Settings resize floor |
| `HELP_WIDTH` / `HELP_HEIGHT` | 520 × 500 | Help window |
| `OVERLAY_WIDTH` / `OVERLAY_HEIGHT` | 336 × 88 | HUD overlay |
| `OVERLAY_MARGIN_X` / `OVERLAY_MARGIN_Y` | 20 / 40 | overlay dock margins |
| `MIXER_OVERLAY_GAP` | 16 | mixer bottom to overlay top |
| `HELP_MARGIN_X` / `HELP_MARGIN_Y` | 24 / 48 | help dock margins |

Component geometry:

| Constant | Value | Usage |
|---|---|---|
| `MIXER_FOOTER_HEIGHT` | 22 | reserved footer band |
| `MIXER_SESSION_LABEL_WIDTH` | 140 | session label column |
| `MIXER_SESSION_ROW_HEIGHT` | 30 | label + hint block |
| `MIXER_CONTROLS_WIDTH` | 62 | mute/reset control column |
| `HOTKEY_LABEL_WIDTH` | 210 | hotkey label column |
| `HOTKEY_ROW_HEIGHT` | 44 | hotkey row card |
| `HOTKEY_CHIP_MIN_WIDTH` | 120 | binding chip minimum |
| `HOTKEY_BUTTON_WIDTH` | 46 | Ghi/Xoá buttons |
| `PICKER_WIDTH` / `PICKER_HEIGHT` | 320 / 180 | running-app picker |

Implementation map:

- `src/style.rs` — colors, spacing, radii, typography, control sizes, surface
  geometry, component geometry, motion durations.
- `src/placement.rs` — dock math; re-exports the surface geometry tokens.
- `src/surfaces/mixer.rs` — footer, rows, SignalRail.
- `src/surfaces/overlay.rs` — HUD layout.
- `src/surfaces/help.rs` — help groups and footer.
- `src/settings/views.rs` — tabs, hotkey rows, picker.
