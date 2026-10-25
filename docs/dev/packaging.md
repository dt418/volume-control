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
dispatch. Use an `egui-dryrun-<n>` tag for dry runs: it exercises packaging
without triggering the Release workflow, which only accepts semver `v*` tags
and fails closed otherwise, verifies the artifacts, and uploads them to the workflow run. Release
attachment is gated behind the repository variable `EGUI_RELEASE_PUBLISH=true`
so the current Tauri release stays authoritative until a dry run and the
signing decision are done.

`.github/workflows/egui-host.yml` runs unit tests, clippy, and fmt on
Windows/Linux/macOS for every change to the Rust crates.

## Signing

Where each credential comes from (costs, CA/B Forum hardware rules, exact
commands): [`signing-secrets.md`](signing-secrets.md).

| Secret | Platform | Purpose |
|---|---|---|
| `WINDOWS_CERT_PFX_BASE64` + `WINDOWS_CERT_PASSWORD` | Windows | base64 .pfx imported for `signtool` (inner exe + setup, timestamped, `/pa` verified) |
| `WINDOWS_CERT_THUMBPRINT` | Windows | alternative: certificate already installed in the store |
| `MACOS_SIGN_IDENTITY` + `MACOS_CERT_P12_BASE64` + `MACOS_CERT_PASSWORD` | macOS | Developer ID import into a temporary keychain and hardened-runtime signing |
| `MACOS_NOTARY_PROFILE` or `MACOS_APPLE_ID` + `MACOS_TEAM_ID` + `MACOS_APP_PASSWORD` | macOS | `notarytool submit --wait` + `stapler staple` |
| `GPG_KEY_ID` + `GPG_PRIVATE_KEY_BASE64` | Linux | import the base64 key and attach a detached ASCII signature for `SHA256SUMS.txt` |

Every step degrades to a logged skip when its secrets are absent, so local and
fork builds stay unsigned but complete. When the release gate is enabled
(`EGUI_RELEASE_PUBLISH=true`), the workflows pass `REQUIRE_SIGNING=true`:
missing signtool/credentials, an unset Developer ID, or missing notarization
credentials then fail the build instead of shipping unsigned artifacts. Verify
signatures after a real run:

```powershell
pwsh scripts/sign-windows.ps1 -Files <setup.exe>,<host.exe> -VerifyOnly
```

## Enabling release upload

Artifacts upload to the workflow run unconditionally; attaching them to the
GitHub release requires the repository variable:

```bash
gh variable set EGUI_RELEASE_PUBLISH --body true --repo dt418/volume-control
```

Publishing refuses artifacts while `WINDOWS_ALLOW_UNTRUSTED_SIGNATURE=true` or
`MACOS_SIGNING_DISABLED=true`; a semver tag fails at the `Refuse untrusted
artifacts` step unless `ALLOW_UNTRUSTED_RELEASE=true` is set as an explicit
waiver for a private/test release.

While `MACOS_SIGNING_DISABLED=true`, macOS artifacts are signed ad-hoc and the
job logs a warning; Windows Authenticode (a self-signed certificate is allowed
only together with `WINDOWS_ALLOW_UNTRUSTED_SIGNATURE`) and the Linux GPG
signature stay mandatory.

The variable is enabled on this repository (2026-10-25) together with
fail-closed signing enforcement. A tag dry run (`v0.1.11-egui-dryrun8`) proved
that no artifact can attach without signatures:

| Platform | Gate failure |
|---|---|
| Windows | `REQUIRE_SIGNING=true but no WINDOWS_CERT_* credentials are set` |
| Ubuntu | `REQUIRE_SIGNING=true but GPG_KEY_ID is not set` |
| macOS | `REQUIRE_SIGNING=true but MACOS_SIGN_IDENTITY is not set` |

`Attach to release` stayed skipped, no GitHub release was created, and the tag
was deleted. Real `WINDOWS_CERT_*` / `MACOS_*` / `GPG_*` secrets are still
required before a signed tag can attach native artifacts; the Tauri release
stays authoritative until then.

## Not automated

- Signing credentials themselves (they live in repository secrets).
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
