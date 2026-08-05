# Mixer Layout and Held Hotkeys Design

## Goal

Fix the Windows mixer button clipping and restore held volume hotkey repeat
for the configured modifier plus Up/Down, without changing the already-merged
Signal Glass value/focus-ring layout.

## Findings

- The mixer is a fixed 400x224 logical card in `crates/volumectl/src/mixer.rs`.
- The reset button currently occupies only x=228..384 (156 px), while its
  native label is `Reset volume to 50 percent`; the label is clipped in the
  supplied screenshot.
- `RegisterHotKey` currently includes `MOD_NOREPEAT` for every action.
- The CapsLock low-level hook suppresses repeated keydown events for every
  action, although only volume changes should repeat.
- The current project already supports modifier + mouse-wheel volume changes;
  this change does not add a separate middle-mouse action.

## Design

### Mixer layout

Keep the 400x224 card and 16 px content margins. Use a two-column button row:
Mute is 132 px wide, the gap is 16 px, and Reset starts at x=164 and fills to
x=384 (220 px). Move the 28 px value row to y=60..88, leaving 26 px before
the slider hit area at y=114 and keeping the focus ring away from the text.
Add pure layout regression tests proving the buttons remain inside the card,
do not overlap, and the value has at least 8 px of air before the slider.

### Held volume hotkeys

Add a pure `is_repeatable_hotkey_id` policy for the four volume actions. The
normal `RegisterHotKey` path omits `MOD_NOREPEAT` only for those IDs, allowing
Windows keyboard auto-repeat for Ctrl/Alt/Ctrl+Alt plus Up/Down. Mute, reset,
mixer, and menu keep `MOD_NOREPEAT`.

The CapsLock hook keeps its repeat suppression for non-volume actions but
posts every matching keydown for the four volume IDs. This preserves one-shot
semantics for command shortcuts while making held volume changes consistent
across the supported modifier paths.

## Verification

- Unit tests fail before the implementation and pass after it.
- `cargo fmt --all --check`.
- `cargo test -q` on Windows.
- `cargo build --release` on Windows.
