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
