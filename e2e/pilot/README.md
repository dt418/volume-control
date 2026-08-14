# Tauri Pilot debug workflow

Tauri Pilot is a local, debug-only inspection and replay tool. WebdriverIO is
the release gate; Pilot scenarios are exploratory/diagnostic evidence and do
not replace the WDIO specs or global-hotkey host probes.

## Requirements

- Tauri v2 and a desktop WebView runtime.
- Rust 1.95.0 or newer. The runner checks `rustc --version` and refuses to
  fall back to the production toolchain.
- The documented CLI installed on `PATH`:

```text
cargo install tauri-pilot-cli --locked
tauri-pilot --version
```

The runner checks the CLI before changing any project files. If it is missing,
execution exits nonzero with the install command above; it never silently
skips.

## Run a scenario

From the repository root:

```text
npm run test:pilot-contract --prefix e2e/tauri
npm run test:pilot --prefix e2e/tauri -- scenario=mixer
npm run test:pilot --prefix e2e/tauri -- scenario=settings
npm run test:pilot --prefix e2e/tauri -- scenario=help
npm run test:pilot --prefix e2e/tauri -- scenario=recovery
```

The runner builds a debug-only `e2e-pilot` binary through the capability and
frontend restore wrappers, starts Vite preview, launches one app process with
an isolated `VOLUMECTL_CONFIG_DIR`, waits for `tauri-pilot ping`, then runs:

```text
tauri-pilot --window window-mixer run e2e/pilot/scenarios/<name>.toml --junit <artifact-path>
```

Temporary `pilot:default`, `withGlobalTauri`, and guest-bridge files are
removed in `finally`. JUnit and screenshots are written under `output/`.
The runner also invokes `tauri-pilot logs --level error` after the scenario;
that output is diagnostic only and does not replace WDIO console-error checks.

## CLI/MCP attachment

Pilot auto-discovers the debug instance. For multi-window inspection use
`--window window-mixer`, `--window window-settings`, or `--window window-help`.
The equivalent local MCP configuration is:

```json
{
  "mcpServers": {
    "tauri-pilot": {
      "command": "tauri-pilot",
      "args": ["--window", "window-mixer", "mcp"]
    }
  }
}
```

Never attach Pilot to a release binary. Do not use Pilot's synthetic X11
keypresses as global-shortcut evidence; use the existing host IPC/probe tests.
