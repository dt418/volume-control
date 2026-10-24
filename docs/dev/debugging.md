# Debugging the egui host

## Fast loop

```powershell
# rebuild + restart on every edit under the two UI crates
powershell -File scripts/dev.ps1
# custom surface and flags
powershell -File scripts/dev.ps1 -Args '--surface=settings','--debug-ui'
```

eframe keeps GPU/UI state in-process, so this restarts the app (≈1 s) rather
than hot-swapping the library. State that matters lives in `config.ini`, so a
restart is cheap.

## Debug UI

- `--debug-ui` or press **F12** at runtime.
- Enables egui `DebugOptions`: hover inspection, interactive widget rects,
  widget hit boxes, focused-widget outline (`style.debug.*`).
- Combine with `RUST_LOG=info,volumecontrol_egui=debug` and
  `RUST_BACKTRACE=1`.

## VS Code

`.vscode/launch.json` ships three configurations:

| Name | Use |
|---|---|
| Egui host (MSVC debugger, debug UI) | breakpoints in Rust on Windows |
| Egui host (CodeLLDB, debug UI) | LLDB (all platforms with the extension) |
| Attach to running egui host | attach to an already launched process |

Build task: `cargo build -p volumecontrol-egui`.

## Automating the app

| Script | Purpose |
|---|---|
| `scripts/find-tray-icon.ps1` | locate the tray icon by UIA tooltip (`Volume Control (egui)`); `-Open`, `-Left`, `-Right` |
| `scripts/app-uia.ps1` | list app windows, dump the accessible tree, invoke a control by name, or drive the tray menu by item index |
| `scripts/capture-window.ps1` | bring one HWND forward and save a PNG (`-Hwnd … -Out …`) |

```powershell
scripts\app-uia.ps1 -Windows
scripts\app-uia.ps1 -TrayIndex 4        # 4 = Settings
scripts\app-uia.ps1 -Click "Đóng"
```

egui content inside child viewports is not yet published through
AccessKit (eframe publishes the root viewport tree only), so window-level UIA
plus pixel/AX tooling is the reliable route for Settings/Help.

## Platform notes

- Windows: UIA via PowerShell (`UIAutomationClient`); Rust can use `windows`
  or `windows-sys`. Elevated windows need an elevated driver.
- Linux: AT-SPI (`atspi`/`zbus`) for semantics; compositor blur is controlled
  by KWin/GNOME extensions, not the app.
- macOS: AX API (`AXUIElement`, `objc2-app-kit`); Screen Recording permission
  is required for pixels, Accessibility permission for AX.
