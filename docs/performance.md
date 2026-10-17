# Performance notes

Measured, not guessed. Every number below was produced by the stage trace in
the Tauri host; re-run it yourself after any change to the startup path.

## Startup stage trace

`VOLUMECTL_STARTUP_TRACE=1` makes the Tauri host log one line per startup
stage with elapsed milliseconds (relative to the start of Tauri `setup`):

```powershell
$env:VOLUMECTL_STARTUP_TRACE = "1"
.\target\debug\VolumeControl.exe   # or target\release
```

Stages, in order: `setup begin`, `single instance`, `config loaded`,
`native surfaces` (wheel bridge + capability probe + volume overlay + tray),
`audio + core` (WASAPI backend + global hotkey registration + AppCore),
`state managed`, `poll threads`.

### Baseline (debug build, Windows 11, 3 runs)

| Stage            | Run 1 | Run 2 | Run 3 |
|------------------|-------|-------|-------|
| config loaded    | 28 ms | 1 ms  | 4 ms  |
| native surfaces  | 74 ms | 37 ms | 35 ms |
| audio + core     | 113 ms| 70 ms | 74 ms |
| poll threads     | 113 ms| 70 ms | 74 ms |

Run 1 includes first-run config creation on disk; later runs show the
steady-state cost. The two hot stages are working as designed:

- `native surfaces` builds the tray menu, the volume overlay window, and the
  wheel-bridge window eagerly. The overlay must exist before the first
  hotkey arrives, so it cannot be lazy without losing the instant volume
  feedback that is the app's core interaction.
- `audio + core` enumerates the default WASAPI endpoint and registers the
  global shortcuts. Both are required before the first action can be served.

Notable measurement: a second process exits within ~1 ms at the
`single instance` stage (named-mutex guard), so a duplicate launch costs
essentially nothing.

### Checked and intentionally not changed

- **DPI awareness**: the embedded manifest carries only the Common-Controls
  dependency. No per-monitor DPI manifest entry was added because `tao`
  calls `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)`
  at event-loop creation (verified in the vendored `tao-0.35.3` source), so
  the process is already per-monitor v2 aware before any of our windows
  exist.
- **Update check**: the network fetch runs on a worker thread and never
  holds the core lock (see `dispatch_tray_command`); the overlay shows an
  immediate "Checking for updates…" card in the active language while the
  request is in flight.

## Interactive latency

The hotkey → volume-change path is covered by the existing probe:
`pwsh -NoProfile -File scripts/verify-hotkey-latency.ps1 -Release -Iterations 10 -OutputRoot output/manual/hotkey-latency`
(sends the configured `open_mixer` shortcut with `keybd_event` and reports
real OS integration timings). Run it on a real desktop session, not inside a
hosted runner.
