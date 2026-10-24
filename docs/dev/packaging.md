# Native packaging (egui host)

The egui host packages without Tauri:

| Platform | Script | Output |
|---|---|---|
| Windows | `scripts/package-egui.ps1` | `VolumeControl_<ver>_x64-setup.exe` (NSIS, per-user; updater contract), portable `.zip`, `SHA256SUMS.txt` |
| Linux | `scripts/package-egui.sh` | `volumecontrol-<ver>-linux-amd64.tar.gz` (binary + `.desktop` + icon), `SHA256SUMS.txt` |
| macOS | `scripts/package-egui.sh` | `volumecontrol-<ver>-macos.zip` (`VolumeControl.app`), `SHA256SUMS.txt` |

Version defaults to `src-tauri/tauri.conf.json` until versioning moves into the
Rust workspace.

## CI

`.github/workflows/egui-release.yml` builds all three on tag pushes and manual
dispatch, verifies the artifacts, and uploads them to the workflow run. Release
attachment is gated behind the repository variable `EGUI_RELEASE_PUBLISH=true`
so the current Tauri release stays authoritative until a dry run and the
signing decision are done.

`.github/workflows/egui-host.yml` runs unit tests, clippy, and fmt on
Windows/Linux/macOS for every change to the Rust crates.

## Not automated

- Code signing (Windows Authenticode, macOS Developer ID + notarization).
- Linux `.deb`/`.rpm`/AppImage; the tarball installs per user.
- Winget manifest regeneration for the NSIS installer.

Signing/notarization and winget require credentials and a real tag; they stay
manual until the first verified native release.

## Verified locally (2026-10-25)

- `scripts/package-egui.ps1` → `VolumeControl_0.1.11_x64-setup.exe` (4.8 MB) +
  portable zip (6.3 MB) + `SHA256SUMS.txt`.
- Silent install into a temp dir produced the app and uninstaller; silent
  uninstall removed both, the desktop shortcut, and the HKCU uninstall key.
- Release binary `target/release/volumecontrol-host.exe --smoke --smoke-ms=4000`
  exits 0; the CLI reports the live volume.
- Release builds ignore `--debug-ui`/F12 (`Style::debug` exists only with
  `debug_assertions`); a warning is logged.
