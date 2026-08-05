# Global keyboard shortcuts

`volumectl` uses `rdev::listen` for keyboard input on Windows, macOS, and
Linux. The listener reads `Config.modifier` at startup and on every config
reload; it does not embed `Ctrl+Alt` in the event loop. The configured
`volume_step` and `volume_step_large` values are also applied by the host when
an action is received. The origin key layout remains `Up`/`Down`, `Shift` for
the large step, `M`, `R`, and `V`.

## Hold-to-Repeat

The first `Up`/`Down` press emits immediately. A worker then emits that same
volume action every 50 ms using `AtomicBool`/`AtomicU8` state and a condition
variable, so it does not busy-spin. Releasing any modifier, `Shift`, or arrow
key clears the hold immediately. `M`, `R`, and `V` are one-shot actions even if
the operating system reports repeated key-press events.

## Permissions and session requirements

- Windows normally needs no extra permission for the global listener. The
  application still needs a normal interactive desktop session; services and
  elevated/security-isolated desktops may not receive the same events.
- macOS requires Accessibility permission for the running app or executable:
  **System Settings → Privacy & Security → Accessibility**. If the binary or
  app bundle changes, macOS may require removing and adding it again. Without
  permission, `rdev` can start but keyboard callbacks may be silent.
- Linux `rdev::listen` uses X11. An X11 `DISPLAY` session and access to that
  display are required. The default build does not claim Wayland support:
  `rdev`'s optional `unstable_grab`/evdev path can work under Wayland, but it
  intercepts input and requires root or membership in the appropriate
  `input`/`plugdev` group. It is intentionally not enabled by default.

`rdev::listen` is blocking and has no portable unlisten API. The repeat worker
is stopped and joined during shutdown; the listener callback is disabled by a
stop flag and the OS removes the native hook as the process exits.
