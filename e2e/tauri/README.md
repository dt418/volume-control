# Tauri E2E package

This package is developer/CI tooling only. It is intentionally separate from
`frontend/package.json` and never belongs in the production bundle.

## Pinned tooling

- `@wdio/tauri-service`: 1.3.0
- `@wdio/cli`, local runner, Mocha framework, spec reporter, types, and
  `webdriverio`: 9.30.1
- Tauri crates: `tauri-plugin-wdio` 1.3.0 and
  `tauri-plugin-wdio-webdriver` 1.3.0
- Optional Pilot crate: `tauri-plugin-pilot` 0.7.2, built with default
  features disabled (the Rust 1.95+ requirement applies only to Pilot builds).

## Debug capability permissions

The source capabilities live here and are copied temporarily by
`prepare-debug-capabilities.mjs`; they must not be placed in
`src-tauri/capabilities` permanently:

- `e2e-wdio.json`: `wdio:default` and `wdio-webdriver:default`
- `e2e-pilot.json`: `pilot:default`

Plugin registration requires both the matching Cargo feature and
`VOLUMECTL_E2E_DEBUG=1`. Production builds ignore the marker. The exclusion
contract is checked with:

```text
npm run test:production-exclusion
node prepare-debug-capabilities.mjs --provider wdio --check-only
```

Build the debug binary with the same temporary capability/config wrapper before
running the browser suite:

```text
node prepare-debug-frontend.mjs -- node prepare-debug-capabilities.mjs --provider wdio -- cargo build -p volumecontrol-tauri --no-default-features --features e2e-wdio
```

The guest bridge is injected only into `frontend/dist` for a debug command.
Debug E2E also starts the Vite preview server on `127.0.0.1:1420`, matching
Tauri's `devUrl`; the server is terminated automatically after the run:

```text
node prepare-debug-frontend.mjs --check-only
npm run test:e2e:debug -- --surface all
```

The script restores every HTML file and removes the copied bridge in `finally`.
The normal frontend package and production build never import the bridge. Each
surface is started in its own isolated WebDriver session so opening a second
Tauri WebView cannot block the originating WebView event loop.

Cross-shell fail-closed wrappers are available from the repository root:

```text
pwsh -NoProfile -File scripts/verify-tauri-e2e.ps1 -Surface all
bash scripts/verify-tauri-e2e.sh --surface all
```

Both wrappers reject a missing binary/dependency and verify that temporary
capabilities and the guest bridge are gone after the run. The recovery surface
uses the debug-only `VOLUMECTL_E2E_BOOTSTRAP_FAILURE=1` fault marker; release
builds ignore it.

Hosted runners do not guarantee a physical audio endpoint. The debug Tauri
runner therefore sets `VOLUMECTL_E2E_AUDIO=virtual`; when combined with
`VOLUMECTL_E2E_DEBUG=1`, the host uses an in-memory endpoint for deterministic
IPC/event/UI volume and mute assertions. The endpoint is compiled out of
release builds, so native backend behavior remains the production authority.
