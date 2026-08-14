# Tauri UI Rendering Recovery Design

**Date:** 2026-08-14
**Status:** Proposed for user review
**Scope:** Mixer, Settings, and Help Tauri webview surfaces on Windows, with
cross-platform-safe layout and lifecycle behavior

## Problem

After Mixer, Settings, and Help migrated from native surfaces to Tauri
webviews, their logical window sizes were restored to the final legacy values,
but their React layout and window lifecycle were not restored as a coherent
unit. The result can be hard to read, clipped, initially drawn at the wrong
position, blank during bootstrap, or perceived as crashed.

The current implementation has three concrete mismatches:

1. `WindowManager::open` builds a visible window before applying its final
   physical size and work-area placement. The user can see an OS-default
   position or an uninitialized webview before the window is ready.
2. Settings and Help use document-style `min-h-screen`/`sticky` layouts inside
   fixed legacy client sizes. This does not guarantee that headers, footers,
   and scroll regions remain inside the viewport.
3. Mixer keeps the compact legacy `400x224` size while adding search, system
   output, and per-application rows. Without an explicit fixed shell and one
   bounded scrolling region, the compact surface becomes visually dense and
   fragile.

Existing tests cover command behavior and pure placement arithmetic, but do
not prove that the packaged desktop app renders, stays alive, remains legible,
or appears at its final location.

## Locked Legacy Contract

The user confirmed that placement and dimensions must match the final native
Windows surfaces immediately before the Tauri migration:

| Surface | Logical client size | Placement |
|---|---:|---|
| Mixer | `400x224` | Bottom-right of the hosting monitor work area, sharing the overlay right edge and sitting exactly 16 physical pixels above it |
| Settings | `760x620`, minimum `620x520` | Centered and clamped to the hosting monitor work area |
| Help | `520x500` | Bottom-right with 24 physical pixels right margin and 48 physical pixels bottom margin |

All calculations use monitor work areas rather than full monitor bounds. DPI
conversion happens exactly once from logical design units to physical pixels.
The Mixer retains the legacy top clamp when the overlay-plus-mixer stack is
taller than the work area.

## Chosen Approach

Preserve the legacy geometry and visual hierarchy while retaining the useful
Tauri-era features. Do not enlarge the three windows merely to make the
current document layouts fit, and do not remove the per-application mixer or
configuration functionality.

### 1. Place before show

Create every webview hidden. Apply final physical size before final physical
position. Keep it hidden while the frontend bootstraps and applies the
Rust-resolved appearance. The frontend then sends a ready signal; Rust
reapplies size and placement, shows the window, and focuses it when required.

If bootstrap fails, the frontend renders a readable error state and still
sends ready, so the window is visible instead of looking like a silent crash.
Duplicate ready events are harmless. Opening an already-live Settings or Help
window reapplies legacy placement and brings it to the foreground rather than
returning as an invisible no-op.

### 2. Fixed surface shells

Each React entry owns a viewport-bounded shell (`height: 100dvh`, no page-level
overflow) with explicit header, body, and footer regions. Only the designated
body/list region scrolls.

- **Mixer:** compact legacy header and always-visible System Output rail;
  search and per-app rows live in the remaining bounded scrolling region.
  Controls keep readable hit targets and the system row never scrolls away.
- **Settings:** restore the legacy title/subtitle/close header, desktop
  navigation rail, responsive narrow selector, one scrolling section pane,
  and always-visible status/action footer.
- **Help:** restore the legacy header and close action, compact shortcut rows,
  one scrolling shortcut body, and always-visible footer.

Shared colors continue to come from the Rust appearance payload. Every surface
has an opaque-enough fallback background and foreground contrast before
backdrop blur or transparency is considered. A stale cached theme cannot be
the only source of first-paint colors.

### 3. Diagnostic startup and evidence script

Add a narrowly scoped diagnostic startup selector used by the verification
script to open exactly one surface without relying on the interactive tray.
Normal launches remain unchanged. The diagnostic path uses the same
`WindowManager` and frontend entries as production; it is not a mock renderer.

Add `scripts/verify-tauri-surfaces.ps1`. For Mixer, Settings, and Help it must:

1. launch the packaged Windows binary with an isolated config directory and
   the requested diagnostic surface;
2. wait on window/process conditions rather than fixed sleeps;
3. record PID, process exit status, window title/handle, client and outer
   bounds, monitor work area, DPI, and expected bounds;
4. capture a PNG of the real top-level window even when occluded;
5. verify that the process remains alive after the window becomes ready;
6. fail on a missing/blank window, invalid bounds, placement mismatch,
   capture failure, or early process exit;
7. terminate only the process it launched and preserve all artifacts.

Artifacts go under `output/tauri-surface-evidence/<before|after>/<surface>/`.
The `before` run captures the current failure state before production fixes;
the `after` run uses the same script and assertions after the fix. The output
directory remains untracked.

## Testing Strategy

Follow red-green TDD:

1. Add failing Rust tests for the missing Mixer top clamp, surface-ready state
   transition, idempotent ready handling, and re-placement of an already-open
   surface.
2. Add failing frontend tests for bounded shell structure, visible
   header/footer contracts, bootstrap error rendering, and exactly one ready
   notification after appearance is applied.
3. Write the PowerShell verifier and run its `before` capture against the
   unfixed implementation. Preserve failure output and screenshots.
4. Implement the smallest Rust lifecycle and React shell changes that satisfy
   the tests.
5. Build the packaged application and run the same verifier into `after`.
6. Inspect every PNG, not only the script exit code. Reject unreadable text,
   clipping, blank content, missing controls, or unexpected transparency.
7. Run frontend tests/build, targeted Rust tests, the full repository gate,
   records self-tests, and the required review workflow.

## Acceptance Criteria

- All three surfaces render non-blank content and remain alive through the
  scripted observation interval.
- No uncaught frontend error, Rust panic, or early process exit appears in the
  captured logs.
- Mixer, Settings, and Help match the locked legacy geometry at 100% scaling;
  DPI-scaled expectations and negative-origin monitor math remain correct.
- Mixer System Output, Settings actions, and Help footer remain visible at the
  minimum supported viewport.
- Text and controls are readable in light and dark appearance fallbacks.
- A surface is never shown at its OS-default location before final placement.
- The evidence script exits nonzero for deliberate missing-window,
  wrong-geometry, blank-capture, and process-crash fixtures.
- `feature_list.json` and `claude-progress.md` record exact commands and the
  before/after artifact paths in the same change set as the code.

## Out of Scope

- Replacing React/Tauri or reverting the migration.
- Changing the native HUD overlay geometry.
- Persisting arbitrary user-moved window positions across launches.
- Redesigning audio, hotkeys, tray behavior, or config schemas.
- Enlarging the legacy surfaces as a substitute for fixing layout containment.
