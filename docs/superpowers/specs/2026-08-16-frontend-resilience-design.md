# Frontend Resilience & Save Transparency Design

**Status:** Approved (Plan B of the review-findings decomposition)

**Goal:** Make frontend failure paths observable and honest: `surface_ready`
errors are logged and retried once instead of silently swallowed, and the
Settings Save flow no longer reports "nothing saved" when the config was
already persisted and only the registry auto-start side-effect failed.

**Background:** The Session 077 fixes made webviews load reliably, but the
follow-up review found two frontend gaps: `markSurfaceReady()` catches every
error with `.catch(() => undefined)` (a hidden window with no diagnostic), and
`SettingsSurface.save()` treats a `set_autostart` rejection as a full save
failure even though `update_settings` already committed the config — the UI
keeps the draft dirty and shows an error although the config is saved.

## Scope

`frontend/src/lib/surface.ts` and `frontend/src/settings/SettingsSurface.tsx`
(plus their vitest suites). No Rust changes.

## Requirements

### R1 — `markSurfaceReady` retry + diagnostics

- `frontend/src/lib/surface.ts`: keep the `Promise<void>` signature and the
  callers' `.finally(() => void markSurfaceReady())` usage.
- On first rejection: `console.warn("[surface] surface_ready failed; retrying
  in 1s:", error)` and retry once after 1000 ms.
- If the retry also rejects: `console.warn("[surface] surface_ready failed
  again:", error)` — no further retries, no unhandled rejection, no UI state
  added (the backend already logs `show ... failed`).

```ts
export function markSurfaceReady(): Promise<void> {
  const ready = () => invoke<void>("surface_ready");
  return ready().catch((error) => {
    console.warn("[surface] surface_ready failed; retrying in 1s:", error);
    return new Promise<void>((resolve) => {
      setTimeout(() => {
        ready().then(resolve).catch((retryError) => {
          console.warn("[surface] surface_ready failed again:", retryError);
          resolve();
        });
      }, 1000);
    });
  });
}
```

### R2 — Save reports partial auto-start failure honestly

- `frontend/src/settings/SettingsSurface.tsx`, `save()`:
  - `update_settings` rejection: unchanged (field errors, draft kept).
  - `set_modifier` rejection: unchanged (caught by the outer `catch`, draft
    kept — modifier belongs to the config).
  - `set_autostart` rejection: no longer blocks `setConfig(draft)`. The flow
    commits the config, sets `status` to `"Saved"`, then runs `set_autostart`
    as a side effect; on rejection it sets
    `status` to `"Saved — auto-start failed: <message>"` without re-dirtying
    the draft.

```ts
await invoke("update_settings", { patch: buildPatch(draft) });
if (draft.modifier !== config.modifier) {
  await invoke("set_modifier", { modifier: draft.modifier });
}
setConfig(draft);
setStatus("Saved");
if (Boolean(draft.autostart) !== Boolean(config.autostart)) {
  try {
    await invoke("set_autostart", { enabled: Boolean(draft.autostart) });
  } catch (error) {
    setStatus(`Saved — auto-start failed: ${String(error)}`);
  }
}
```

### R3 — Bootstrap syncs the switch with the registry

- `SettingsSurface.tsx` bootstrap: when `get_autostart` resolves, update the
  committed config AND the draft so the switch reflects the real registry
  state instead of the stale INI `autostart` preference (the INI never
  persists `autostart`; `set_autostart` owns the registry).

```ts
void invoke<AutostartStatus>("get_autostart")
  .then((autostart) => {
    if (disposed) return;
    setAutostartDisabled(false);
    setConfig((current) =>
      current ? { ...current, autostart: autostart.enabled } : current,
    );
    setDraft((current) =>
      current ? { ...current, autostart: autostart.enabled } : current,
    );
  })
  .catch(() => {
    if (!disposed) setAutostartDisabled(true);
  });
```

- Bootstrap runs once on mount before the user can edit, so this cannot
  overwrite a user edit; after any Save, the committed config already carries
  the latest draft value.

## Testing

- `npm test` (frontend) must pass, including:
  - `surface.test.ts`: `markSurfaceReady` resolves after a first rejection
    (retry succeeds) and after two rejections (logs, still resolves).
  - `SettingsSurface.test.tsx`: `set_autostart` rejection after a successful
    `update_settings` shows `Saved — auto-start failed` and disables Save;
    success path still shows `Saved` and disables Save; `get_autostart`
    resolving to `enabled: true` flips the switch on from an initial
    `autostart: false` bootstrap config.
- Existing 92 frontend tests stay green; `npm run build` (tsc) passes.
- E2E `verify-tauri-e2e.ps1 -Surface settings` stays green.

## Acceptance Criteria

- No silent `surface_ready` failures: every failure is logged, and one retry
  is attempted.
- After a successful `update_settings`, the UI always shows the committed
  config as saved even when the auto-start registry write fails.
- Draft dirty state, field errors, and modifier failures behave exactly as
  before.

## Residual Risks

- The retry window (1s) can delay `surface_ready` success by up to 1s in the
  rare failure case; window visibility waits for the retry.
- `set_autostart` is not re-run automatically after a partial failure; the
  user toggles it again and saves (the switch reflects the draft value).
