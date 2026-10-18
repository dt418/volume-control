# Winget Deployment Pipeline with REA Static PE Verification

## Problem Statement

VolumeControl provides Windows NSIS installers (`VolumeControl_<version>_x64-setup.exe`), but deploying to Microsoft's official Windows Package Manager repository (`microsoft/winget-pkgs`) requires:
1. Strict schema-compliant manifests (v1.12.0) matching the installer's actual runtime behavior.
2. Verified binary attributes (installer type, execution level, silent switches, uninstallation keys, x64 architecture).
3. Exact SHA-256 hashing computed from the official released artifact.
4. Validation via `winget validate` and local installation testing before publishing.
5. Tracking upstream submission lifecycle on `microsoft/winget-pkgs` (PR #454508) and updating public landing documentation accordingly.

## System Architecture

```
[GitHub Release v0.1.7]
         │
         ▼
[VolumeControl_0.1.7_x64-setup.exe]
         │
         ├────────────────────────────────────────┐
         ▼                                        ▼
[scripts/verify-installer-rea.mjs]       [scripts/generate-winget-manifest.sh]
(REA Static Binary Inspection)                      │
 - PE Header & Machine Type (PE32/x64)              ▼
 - NSIS Signature (NullsoftInst)          [winget/dt418.VolumeControl/0.1.7/]
 - Elevation Manifest (asInvoker)          - dt418.VolumeControl.yaml
 - Switches (/S, /D)                       - dt418.VolumeControl.installer.yaml
 - Registry Uninstall Key                  - dt418.VolumeControl.locale.en-US.yaml
 - SHA-256 Calculation                              │
         │                                          ▼
         │                                [winget validate --manifest]
         │                                          │
         │                                          ▼
         │                                [winget install --manifest (local)]
         ▼                                          │
         └───────────────────┬──────────────────────┘
                             ▼
               [microsoft/winget-pkgs PR #454508]
                 - Validation: Passed
                 - CLA: Signed
                 - Status: Pending Moderator Review
                             │
                             ▼
                 [docs/index.html & Records]
                 - WINGET_LIVE toggle & docs
                 - feature_list.json (vol-099)
                 - claude-progress.md
```

## Detailed Specification

### 1. REA (Reverse Engineer Anything) Static Binary Inspection

The tool `scripts/verify-installer-rea.mjs` inspects the released binary without executing untrusted code:
- **PE Header Inspection**:
  - Validates `MZ` DOS signature (`0x5A4D`) at offset `0x00`.
  - Resolves e_lfanew pointer to PE signature `PE\0\0` (`0x00004550`).
  - Inspects COFF header: Machine type `0x014c` (i386 NSIS stub loader) or `0x8664` (AMD64).
  - Inspects Optional Header: Subsystem `0x0002` (Windows GUI).
- **NSIS Stub & Payload Verification**:
  - Searches binary for NSIS marker `NullsoftInst` (`0xEFBEADDE` / `NullsoftInst`).
  - Confirms inner packed files target AMD64 architecture.
- **Elevation Requirement (UAC)**:
  - Scans RT_MANIFEST XML embedded in binary for `<requestedExecutionLevel level="asInvoker"`.
  - Confirms installer runs per-user without requiring administrative elevation (matching `Scope: user` in winget manifest).
- **Silent Switch & Command Line Protocol**:
  - Verifies presence of NSIS command line flags `/S` (case-sensitive uppercase) and `/D=<path>`.
- **Uninstaller Registry Contract**:
  - Verifies string reference to `Software\Microsoft\Windows\CurrentVersion\Uninstall\VolumeControl`.
- **Integrity**:
  - Computes upper-case SHA-256 digest: `541C57E8F7B57F571559278E167BE4123923A2CFB22CB776791D84519E30B92B`.

### 2. Winget Manifest Generation

`scripts/generate-winget-manifest.sh` generates the 1.12.0 trio in `winget/dt418.VolumeControl/<version>/`:
- **dt418.VolumeControl.yaml**:
  - PackageIdentifier: `dt418.VolumeControl`
  - PackageVersion: `0.1.7`
  - DefaultLocale: `en-US`
  - ManifestType: `version`
  - ManifestVersion: `1.12.0`
- **dt418.VolumeControl.installer.yaml**:
  - MinimumOSVersion: `10.0.17763.0`
  - InstallerType: `nullsoft`
  - Scope: `user`
  - InstallModes: `[interactive, silent, silentWithProgress]`
  - UpgradeBehavior: `install`
  - Architecture: `x64`
  - InstallerUrl: `https://github.com/dt418/volume-control/releases/download/v0.1.7/VolumeControl_0.1.7_x64-setup.exe`
  - InstallerSha256: `<computed uppercase 64-hex SHA-256>`
- **dt418.VolumeControl.locale.en-US.yaml**:
  - PackageIdentifier: `dt418.VolumeControl`
  - PackageVersion: `0.1.7`
  - PackageLocale: `en-US`
  - Publisher: `dt418`
  - PackageName: `VolumeControl`
  - License: `MIT`
  - Moniker: `volumecontrol`

### 3. Local Verification

1. `winget validate --manifest winget/dt418.VolumeControl/0.1.7` validates schema and field consistency.
2. `winget install --manifest winget/dt418.VolumeControl/0.1.7` validates actual installer invocation.

### 4. Upstream Submission Lifecycle & Web Docs

- Inspect PR #454508 on `microsoft/winget-pkgs`:
  - Upstream URL: `https://github.com/microsoft/winget-pkgs/pull/454508`
  - Pipelines: Azure validation completed and passed.
  - CLA: Signed by maintainer.
  - Status: Pending community moderator review.
- Status synchronization in `docs/index.html`:
  - Current state: Package PR is open and pending merge.
  - Keep `var WINGET_LIVE = false` until PR is merged and indexed by winget CDN.
  - Document the exact condition and verification procedure for flipping `WINGET_LIVE = true`.

### 5. Automated Regression Test Suite

- `scripts/test-winget-deploy.sh`:
  - Tests `scripts/verify-installer-rea.mjs` against valid release artifact, missing file, corrupted PE header.
  - Validates `scripts/generate-winget-manifest.sh` output format.
  - Enforces fail-closed error handling.

## Self-Review

1. **Placeholder scan**: No TBD/TODO; exact hashes, URLs, paths, and commands specified.
2. **Internal consistency**: Architectures, manifests, and script names align across all sections.
3. **Scope check**: Focuses strictly on Winget deployment, REA binary inspection, and upstream tracking.
4. **Ambiguity check**: Clear distinction between local manifest validation (passing) and upstream index availability (pending moderator approval).
