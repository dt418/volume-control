# UI system

Fluent 2-inspired ramp, tuned for a compact Windows-style utility and adapted
per platform. Tokens live in `crates/volumecontrol-egui/src/style.rs`.

## Type ramp

Derived from the user's base font size (`ui_font_size`, default 15):

| Token | Size | Use |
|---|---|---|
| caption | base − 4 | footers, hints ("Esc để đóng") |
| small | base − 2 | subtitles, helper text, field labels |
| body | base | rows, buttons, sidebar |
| title | base + 4 | window titles, panel headers |
| headline | base + 8 | reserved (empty states) |
| display | base + 14 | reserved (overlay percentage) |

Rules:

- Window title: `title` + semibold.
- Panel/section title: `body` + semibold; never larger than the window title.
- Helper text: `small`, muted; contrast ≥ 4.5:1 against `card`.
- Icon-only buttons: 26–28 px hit area, glyph 13–14 px.

## Spacing and geometry

Scale: 2, 4, 6, 8, 12, 16, 24, 32 (`SPACE_1..8`).

| Control | Height |
|---|---|
| Icon button | 26 (mixer) / 28 (settings header) |
| Button | 28 |
| Input / combo | 30 |
| Settings sidebar row | 36 |
| Tray/menu row | 32 |

Radii: 4 (chips), 8 (controls, cards), 12 (window content), 16 (HUD shell).
DWM supplies the outer window radius on Windows 11; egui rounds content.

## Menu ordering

Dropdown and tray rows use a fixed column order so nothing shifts:

```
[check 22px][icon 24px][label][optional shortcut/right hint]
```

The check column is reserved even when unselected.

## Materials

| Mode | Windows | macOS | Linux |
|---|---|---|---|
| Auto | Mica for Settings, Acrylic elsewhere | `UnderWindowBackground` vibrancy | translucent egui colors |
| Translucent | Acrylic tint 180 α | HUD material | translucent colors (compositor may blur) |
| Acrylic | `SetWindowCompositionAttribute` acrylic | `HUDWindow` material | translucent + in-app glass shader |
| Blur | DWM blur-behind | behind-window blur material | compositor blur (KWin/GNOME extension) |
| Mica | `DWMWA_SYSTEMBACKDROP_TYPE` Mica | sidebar material | not available → translucent |
| Transparent | clear backdrops, 90 α tint | clear vibrancy | translucent colors |
| LiquidGlass | Acrylic fallback tint | Liquid Glass (`NSGlassEffectView`, macOS 26+), HUD fallback | in-app `liquid_glass` shader |
| Opaque | all effects cleared | vibrancy cleared | opaque colors |

`window-vibrancy` documents that Linux blur is compositor-controlled; the app
never assumes it. High-contrast or reduced-transparency settings force Opaque
(`ui::capabilities::resolve_material`).
