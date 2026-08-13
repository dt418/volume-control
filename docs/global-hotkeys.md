# Global keyboard shortcuts

`volumectl` registers a fixed set of global hotkey combos with the
`global-hotkey` 0.8.0 crate (`crates/volumectl/src/hotkeys_global.rs`). The
listener reads `Config.modifier` at startup and on every config reload; it
does not embed `Ctrl+Alt` in the event loop. The configured `volume_step`
(default 1, i.e. 1 %) and `volume_step_large` (default 10) values are applied
by the host when an action is received. The origin key layout remains
`Up`/`Down`, `Shift` for the large step, `M`, `R`, and `V`.

## Registration

- **Windows** uses `RegisterHotKey` on a hidden message-only window — no
  low-level keyboard hook and no extra permission. A normal interactive
  desktop session is still required; services and elevated/security-isolated
  desktops may not receive the same events.
- **macOS** uses the Carbon `RegisterEventHotKey` API, which needs **no
  Accessibility permission** (unlike the previous `rdev` CGEventTap).
- **Linux** uses X11 via the pure-Rust `x11rb` XRecord backend. An X11
  `DISPLAY` session with access to that display is required. `global-hotkey`
  0.8 has no Wayland backend, so the Wayland limitation is unchanged from the
  previous backend.

## Combo layout

`MOD` is the configured modifier (`CtrlAlt` by default → `Ctrl+Alt`):

| Action | Combo |
|--------|-------|
| Volume +1 % | `MOD+↑` |
| Volume −1 % | `MOD+↓` |
| Volume +10 % | `MOD+Shift+↑` |
| Volume −10 % | `MOD+Shift+↓` |
| Mute | `MOD+M` |
| Open tray menu | `MOD+Shift+M` |
| Reset to 50 % | `MOD+R` |
| Open mixer | `MOD+V` |

On macOS the `CtrlAlt` config also registers the `⌘+⌥` spelling of every
combo, so both `⌃+⌥` and the macOS-native `⌘+⌥` work. The `CapsLock`
modifier is not expressible in the native registration APIs on any platform;
a `CapsLock` config falls back to the `Ctrl+Alt` combos with a warning logged
at startup.

## Hold-to-Repeat

The first `↑`/`↓` press emits immediately. A worker then emits that same
volume action every 50 ms using `AtomicBool`/`AtomicU8` state and a condition
variable, so it does not busy-spin. `M`, `R`, and `V` are one-shot actions
even if the operating system auto-repeats the held combo.

Release detection is combo-level: the hold ends when the hotkey's main key is
released. Releasing only a modifier early does not end the hold while the
main key stays down — on Windows the release poll checks the hotkey's own key
via `GetAsyncKeyState`.

## Conflicts

Combos are registered per action. If a combo is already owned by another
application, `register` returns `Err(AlreadyRegistered)`; that combo is
skipped with a warning and the remaining combos still register. The Help
surface shows `Conflicted` for actions whose combos could not be registered.

## Lifecycle

The listener thread drains `GlobalHotKeyEvent` events into the host action
channel; hosts poll it with `try_recv` on their existing timers. On shutdown
the repeat worker and listener thread are joined and every registered combo
is unregistered.
