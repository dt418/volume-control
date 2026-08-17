---
name: tauri-windows-host-surfaces
description: Use when adding, changing, or debugging a Tauri webview surface in volume-control — mixer/settings/help/overlay window lifecycle in src-tauri/src/window_manager.rs, surface IPC commands, or the state://* frontend event bridge. Covers the lazy per-surface WebView2 lifecycle, E2E debug markers that disable auto-hide/auto-close, and the macOS/Linux-only webview overlay rule.
---

# Tauri Windows Host Surfaces

VolumeControl's four interactive surfaces are **lazy webview windows** managed by
`src-tauri/src/window_manager.rs`; the app starts with zero webviews to protect
idle RAM. This skill is the surface-architecture guide. For thread-safety and
deadlock invariants use `tauri-deadlock-guard` — the two must be read together
when touching surface code.

## Surface map (single source of truth: `SurfaceId`)

`SurfaceId` (Mixer/Settings/Help/Overlay) owns three parallel contracts that
MUST stay in sync in `window_manager.rs`:

| SurfaceId | label          | title                  | entry (Vite keeps `src/`) |
|-----------|----------------|------------------------|---------------------------|
| Mixer     | `window-mixer`   | Volume Mixer           | `src/mixer/index.html`    |
| Settings  | `window-settings`| VolumeControl Settings | `src/settings/index.html`|
| Help      | `window-help`    | VolumeControl Help     | `src/help/index.html`     |
| Overlay   | `window-overlay` | VolumeControl Overlay  | `src/overlay/index.html`  |

- `label()`, `title()`, `entry()` must all round-trip through
  `SurfaceId::from_label` (unit test `surface_labels_and_entries_are_stable`).
- `SurfaceId::all()` drives the lazy open/close set — add any new surface there
  too, or `from_label` never resolves it.

## Overlay is macOS/Linux-only

`WindowManager::open_impl` **rejects `SurfaceId::Overlay` on Windows** — Windows
routes overlay notifications to its native Win32 HUD (`native_win32` +
`TauriSink::overlay` Windows branch). Never "fix" that guard: creating the
webview overlay on Windows duplicates the HUD.

## Lazy lifecycle

- `open(..)` marshals to the main thread, builds the window hidden
  (`build().visible(false)`), applies placement from the monitor work area,
  then the frontend calls `surface_ready` after bootstrap to `show()`.
- `close(..)` destroys the window and drops it from the `active` set; a
  `Destroyed` event also removes it so a stale entry can never make `open`
  a permanent no-op.
- Reopen when `active` but window missing: the manager recreates instead of
  returning a silent no-op (clicks would look dead).
- Mixer auto-closes on `Focused(false)` in production; under the E2E marker it
  stays open (see below).

## E2E debug markers (do not break them)

The embedded WebDriver attaches tens of seconds after app start. These markers
keep surfaces discoverable/assertable; production releases must be unaffected:

- `VOLUMECTL_E2E_DEBUG=1` (+ `cfg!(debug_assertions)`):
  - disables host overlay auto-hide timer (`events_sink.rs` overlay path),
  - disables mixer auto-close-on-focus-loss
    (`mixer_auto_close_on_focus_loss(e2e_debug)` in `window_manager.rs`),
  - the frontend `e2e_debug` bootstrap flag disables the frontend timer too.
- WebView2 isolation (`wdio.conf.ts`): `WEBVIEW2_USER_DATA_FOLDER=<output>/webview2-<pid>`
  per surface run so orphaned renderer processes from force-killed runs can't
  corrupt the next app's webview ("No window could be found" flake).
- Pre/post-run cleanup kills leftover `VolumeControl.exe` (`scripts/verify-tauri-e2e.ps1`)
  so the embedded WebDriver port 4445 is free and no stale native-HUD ghost
  remains.

## Frontend contract

- Each surface module calls `markSurfaceReady()` after receiving its bootstrap
  payload — otherwise the window stays hidden.
- `state://*` events (`volume`, `sessions`, `hotkeys`, `backend`, `overlay`)
  flow through the shared `lib/ipc.ts` `listen`; the text payload for a
  surface is never fetched directly from an open window.
- Esc-to-close is handled per surface and on Windows the mixer also closes on
  the `close_mixer_request` event (reliable `Focused(false)` vs flaky JS blur).

## Adding a new surface (checklist)

1. Add the variant to `SurfaceId` (+ `label`/`title`/`entry`/`all`/`from_label`).
2. If it needs placement, extend `place_surface` in `window_manager.rs`.
3. Add an `index.html` under the matching `frontend/src/<name>/` (Vite preserves
   the `src/` prefix; keep the entry path in sync).
4. Wire frontend: `markSurfaceReady()` + `state://*` listeners.
5. Add/keep the round-trip unit test for the new label.
6. Run `bash scripts/check-tauri-deadlock.sh` and the full gate, then the E2E
   surface spec (`VOLUMECTL_VERIFY_SURFACE=window-<name>`).

## References

- `src-tauri/src/window_manager.rs` — SurfaceId + placement + on_main + E2E gate.
- `src-tauri/src/commands.rs` — async surface IPC commands.
- `src-tauri/src/events_sink.rs` — `state://*` + overlay routing.
- `e2e/tauri/wdio.conf.ts` — WebView2 isolation + debug markers wiring.
- `.agents/skills/tauri-deadlock-guard/SKILL.md` — threading/deadlock invariants.
