# Cross-platform release evidence checklist

This checklist is the evidence boundary for VolumeControl releases. It records
what was actually exercised on Windows, Ubuntu/Xvfb, WSLg, and hosted macOS;
it does not turn a compile or a headless GUI run into proof of native OS
integration.

Use one checklist copy per release candidate. A check is complete only when the
command, commit SHA, operating-system version, architecture, and evidence path
are recorded. Do not record hosted workflow URLs or artifact paths until that
hosted run has completed.

## Evidence record

Record these fields before inspecting a release:

```text
Release/tag:
Commit SHA:
Date (UTC):
Runner or machine:
OS/version/build:
Architecture (`uname -m` and `file` output):
Display server/topology (Xvfb, WSLg, Wayland, X11, Retina/multi-monitor):
Audio endpoint/backend:
Configured shortcut:
Measured latency (p50/p95 and sample count, when applicable):
Evidence directory:
Hosted workflow URL/run ID (only after completion):
Artifact names and SHA256SUMS path (only after completion):
Reviewer/decision:
```

Use these confidence labels in the review:

- **Strong** — deterministic core/frontend tests, a passing WDIO surface test
  with its JUnit/manifest/log evidence, or a real OS probe with a complete
  report tied to the app SHA.
- **Partial/manual** — native audio, tray/menu-bar behavior, global shortcut
  delivery, permission prompts, display topology, or other behavior that needs
  a real interactive desktop and cannot be inferred from a hosted headless
  job.
- **Compile-only** — a feature compiled or a library probe succeeded, without
  proving runtime behavior (for example optional GTK4 layer-shell).
- **Not claimed** — behavior the environment cannot exercise. Leave it
  explicitly unsupported instead of marking it green.

## 1. Shared deterministic checks

Run from the repository root and retain the command output under the evidence
directory:

```bash
npm ci --prefix frontend
npm test --prefix frontend
npm run build --prefix frontend

npm ci --prefix e2e/tauri
E2E_DRIVER_PROVIDER=embedded node e2e/tauri/test-provider.mjs --provider embedded --platform windows

cargo fmt --all --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
```

The WDIO wrapper must be run with an explicit provider. Its output must contain
JUnit XML, `manifest.json`, `timings.json`, and the frontend/backend logs
expected by the evidence contract. A missing artifact is a failed check.

Windows:

```powershell
$env:E2E_DRIVER_PROVIDER = 'embedded'
pwsh -NoProfile -File scripts\verify-tauri-e2e.ps1 -Surface all -OutputRoot output\tauri-e2e\manual-windows
```

macOS:

```bash
E2E_DRIVER_PROVIDER=embedded bash scripts/verify-tauri-e2e.sh \
  --surface all --output-root output/tauri-e2e/manual-macos
```

Ubuntu/Xvfb:

```bash
E2E_DRIVER_PROVIDER=embedded xvfb-run -a bash scripts/verify-tauri-e2e.sh \
  --surface all --output-root output/tauri-e2e/manual-ubuntu
```

WDIO shortcut cards prove UI/configuration and IPC behavior only. They do not
prove that the operating system delivered a native shortcut.

## 2. Windows interactive evidence

Build the release executable and record its SHA-256 before starting the probe:

```powershell
Get-FileHash target\release\VolumeControl.exe -Algorithm SHA256
Get-ComputerInfo | Select-Object WindowsProductName,WindowsVersion,OsBuildNumber
```

Run the real shortcut-to-mixer probe on a Windows desktop, not in Linux, macOS,
Xvfb, or a synthetic WDIO session:

```powershell
pwsh -NoProfile -File scripts\verify-hotkey-latency.ps1 `
  -Release -Iterations 10 -OutputRoot output\manual\hotkey-latency
```

Inspect both `output/manual/hotkey-latency/hotkey-latency.json` and
`hotkey-latency.txt`. Require the app SHA, OS build, configured shortcut,
keydown timestamp, surface-visible timestamp, every iteration result, and p50
/p95 values. A failed iteration or an orphaned child process fails the probe.

Manually inspect and record, with the same app SHA:

- actual global shortcut registration, conflict/disabled states, and repeat
  behavior;
- tray icon/menu actions, overlay placement and auto-hide, and focus return;
- WASAPI output selection and per-application session behavior;
- config reload, clean startup/shutdown, and autostart when enabled;
- one-monitor and multi-monitor placement, including the selected display and
  scaling factor.

The Windows probe is **Strong** evidence for the observed Windows machine and
build only. The Windows CI build and WDIO run do not substitute for this
interactive evidence.

## 3. Ubuntu 24.04 and Xvfb

Install the same WebKitGTK/GTK development surface used by CI, including both
`pkg-config` probes:

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev libgtk-4-dev libadwaita-1-dev \
  libpulse-dev libx11-dev libxi-dev libxtst-dev libxdo-dev \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  pkg-config build-essential xvfb
pkg-config --modversion gdk-3.0
pkg-config --modversion webkit2gtk-4.1

cargo build --features gtk-renderer
xvfb-run -a cargo test --features gtk-renderer
E2E_DRIVER_PROVIDER=embedded xvfb-run -a bash scripts/verify-tauri-e2e.sh \
  --surface all --output-root output/tauri-e2e/manual-ubuntu
```

If the optional package is available, record the probe and build separately:

```bash
if pkg-config --exists gtk4-layer-shell-0; then
  cargo build --features gtk-renderer,layer-shell
else
  echo 'gtk4-layer-shell unavailable: layer-shell is compile-only here'
fi
```

Ubuntu/Xvfb is **Strong** for deterministic Rust, GTK smoke, webview, and IPC
checks that completed with their evidence. It is **Partial/manual** for tray,
PulseAudio hardware, global hotkeys, and real display geometry. Xvfb does not
prove a Wayland compositor, layer-shell protocol behavior, hardware audio, or
multi-monitor behavior.

Linux tray manual row: on a real desktop with a StatusNotifier host (GNOME
Shell/KDE/XDG desktop), record the desktop session and appindicator behavior:
icon renders, left click opens the menu, volume label tracks live changes,
and the Exit item terminates the process. CI only proves tray creation
attempts without crashing; it cannot prove a tray-hosted menu.

Linux per-app audio manual row: start a Pulse server (`pulseaudio --start`),
play two streams from different apps (e.g. `paplay` + a browser), open the
Mixer and record that both sink-inputs appear as sessions, adjust each
slider and mute toggle, verify the list updates, and stop one stream to
confirm the stale session disappears (the §9.6 stale-id path). Record the
Pulse server version and whether PipeWire's Pulse compat layer was in use.

## 4. WSLg manual boundary

WSLg can provide a real Wayland/X11 desktop for local investigation, but it is
not the hosted Ubuntu release environment and is not a replacement for a
Windows run:

```powershell
wsl --update
wsl -d Ubuntu-24.04
```

Inside the WSLg distribution, record the display context and architecture
before running the native checks:

```bash
uname -a
uname -m
printf 'DISPLAY=%s\nWAYLAND_DISPLAY=%s\nXDG_SESSION_TYPE=%s\n' \
  "${DISPLAY:-}" "${WAYLAND_DISPLAY:-}" "${XDG_SESSION_TYPE:-}"
file target/release/VolumeControl 2>/dev/null || true
cargo test --features gtk-renderer
```

When testing a Wayland path, do not wrap the run in `xvfb-run`; that changes the
display server under test. Record whether the overlay/window actually used
Wayland or X11 and include screenshots/logs in the evidence directory. WSLg is
**Partial/manual** evidence for that WSLg version and configuration; it does not
claim native Ubuntu hardware, a general Wayland compositor, tray support, or
Windows global-shortcut delivery.

## 5. Hosted macOS and local macOS inspection

The hosted macOS job is useful for AppKit/core compilation, renderer smoke
tests, webview/IPC evidence, and package structure. The WDIO wrapper from
section 1 is debug-only: it prepares a debug/plugin binary for surface tests
and never substitutes for production package inspection.

Obtain or build the production package before inspecting `target/release` or
the app bundle. For a local package:

```bash
npm ci --prefix frontend
npm run build --prefix frontend
frontend/node_modules/.bin/tauri build --no-bundle --ci

sw_vers
uname -m
file target/release/VolumeControl
RELEASE_PLATFORM=macos RELEASE_TAG=v0.0.0-manual bash scripts/package.sh
```

For a hosted package, download the completed validation artifact instead of
running a local build:

```bash
gh run download <RUN_ID> --name "validated-macos-<COMMIT_SHA>" \
  --dir output/release/macos
```

Then inspect the downloaded or locally generated package after unzipping it:

```bash
mkdir -p output/manual/macos-package
unzip -q output/release/macos/volumecontrol-<version>-macos.zip \
  -d output/manual/macos-package
# For a local package, use:
# unzip -q dist/volumecontrol-0.0.0-manual-macos.zip \
#   -d output/manual/macos-package
codesign --verify --strict --verbose=2 \
  output/manual/macos-package/VolumeControl.app
plutil -lint \
  output/manual/macos-package/VolumeControl.app/Contents/Info.plist
file output/manual/macos-package/VolumeControl.app/Contents/MacOS/volumectl
```

When only a hosted package was downloaded, inspect its executable with `file`
after extraction; `target/release/VolumeControl` is a local-build path and may
not exist on the inspection machine.

`codesign --verify` and `plutil --lint` prove package structure and the current
signature state. Hosted macOS does **not** prove TCC/Accessibility approval,
menu-bar behavior, hardware CoreAudio routing, Retina or multi-monitor
geometry, or Gatekeeper acceptance. Those are **Partial/manual** checks and
must be exercised on a real macOS desktop when they matter for a release.

macOS menu-bar tray manual row: on a real macOS desktop, record that the menu
bar extra renders, the menu opens on click, the live volume label tracks
changes, and the Exit item quits the app. The OpenMenu hotkey popup is
documented as unavailable on macOS/Linux (Tauri's `TrayIcon` exposes no
programmatic popup API); the menu opens on click only.

The repository currently creates an ad-hoc signature (`codesign --sign -`) for
validation. It is not a public distribution signature and does not include a
Developer ID certificate or notarization ticket. Public macOS distribution is
a future protected workflow requiring Developer ID secrets, temporary
keychain import, `notarytool`, stapling, `spctl`, and cleanup. No certificate,
password, or token belongs in this repository.

## 6. SHA-bound release inspection

The release workflow first validates the tag, then runs the reusable desktop
matrix. The publish job must only promote artifacts named
`validated-<platform>-<commit-sha>`; it must not rebuild an unvalidated binary.
For a completed hosted run, download each artifact and inspect it with the
commit SHA from the workflow:

```bash
gh run download <RUN_ID> --name "validated-ubuntu-<COMMIT_SHA>" --dir output/release/ubuntu
gh run download <RUN_ID> --name "validated-macos-<COMMIT_SHA>" --dir output/release/macos
gh run download <RUN_ID> --name "validated-windows-<COMMIT_SHA>" --dir output/release/windows

bash scripts/verify-release-metadata.sh \
  output/release/ubuntu "<COMMIT_SHA>" ubuntu
bash scripts/verify-release-metadata.sh \
  output/release/macos "<COMMIT_SHA>" macos
bash scripts/verify-release-metadata.sh \
  output/release/windows "<COMMIT_SHA>" windows
```

The verifier requires `build-metadata.json`, the package, `SHA256SUMS.txt`, a
matching `artifact_sha256`, the expected commit SHA, and the expected platform.
This publish-time verifier does not re-verify E2E JUnit, manifest, or platform
logs. Review those validation files separately before approving the release:
confirm each downloaded validation artifact contains its JUnit, manifest, and
platform-log paths before inspecting the package contents without executing
them:

```bash
tar tzf output/release/ubuntu/volumecontrol-<version>-ubuntu.tar.gz
unzip -l output/release/macos/volumecontrol-<version>-macos.zip
unzip -l output/release/windows/volumecontrol-<version>-windows.zip
```

Reject validation when a platform artifact, metadata field, or checksum is
missing. Also reject the release decision when the validation artifact lacks
its required E2E manifest, JUnit report, or platform log; that is a separate
evidence-review gate, not an additional claim about the publish verifier.
Record the final `SHA256SUMS.txt` path only after the publish job completes
successfully.

## Claims boundary

| Area | Strong evidence | Partial/manual or not claimed |
| --- | --- | --- |
| Core/frontend | Rust tests, Vitest, TypeScript/Vite build | None beyond the tested commit |
| Tauri surfaces | WDIO JUnit + manifest + logs for the requested surface | Native OS input/audio/tray behavior |
| Windows | Real hotkey report plus package/startup checks | Other hardware, displays, or OS builds not tested |
| Ubuntu/Xvfb | GTK/webview/IPC/core tests and package metadata | Wayland compositor, tray, hardware PulseAudio, global hotkeys |
| WSLg | The recorded WSLg display path and local smoke run | Hosted Ubuntu equivalence or Windows integration |
| macOS hosted | AppKit/core/webview tests, `codesign`, `plutil`, architecture | TCC, menu bar, CoreAudio hardware, Retina/multi-monitor, Gatekeeper |
| Release | SHA-bound metadata, checksums, package inspection | Public macOS distribution until Developer ID/notarization exists |

Do not claim screenshot diffs, universal macOS support, real Wayland coverage,
hardware audio confidence, or native global-shortcut delivery from an
environment that did not exercise those things.
