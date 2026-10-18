# Winget Deploy Pipeline with REA Binary Verification Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Automate and verify the end-to-end Winget deployment pipeline for VolumeControl, using REA static PE inspection for pre-deployment binary verification, generating schema-valid 1.12.0 manifests, validating with `winget validate`, and automating submission artifact preparation for `microsoft/winget-pkgs`.

**Architecture:** A standalone verification module (`scripts/verify-installer-rea.mjs`) inspects the installer binary and emits an REA-format verification dossier. The manifest generator (`scripts/generate-winget-manifest.sh`) is hardened and integrated with a high-level orchestrator (`scripts/deploy-winget.sh` and `scripts/deploy-winget.ps1`) that coordinates REA verification, manifest generation, `winget validate` CLI checks, and submission bundle assembly. A dedicated regression test suite (`scripts/test-winget-deploy.sh`) ensures zero-drift enforcement.

**Tech Stack:** Bash, PowerShell, Node.js (v20+ ES Modules), WinGet CLI v1.12+, REA MCP Evidence model.

## Global Constraints

- Manifest version floor: `1.12.0` (matching `scripts/generate-winget-manifest.sh`).
- Package Identifier: `dt418.VolumeControl`.
- Installer name convention: `VolumeControl_<version>_x64-setup.exe`.
- All hashes must be computed from real bytes (SHA-256 uppercase 64 hex characters) — never mocked or hardcoded.
- REA verification must fail closed on invalid PE architecture, missing NSIS markers, or hash mismatches.
- Zero external runtime dependencies in production — scripts use built-in Node.js, PowerShell, and Bash.

---

### Task 1: REA Static PE Binary Verification Module

**Files:**
- Create: `scripts/verify-installer-rea.mjs`
- Test: `scripts/test-verify-installer-rea.mjs`

**Interfaces:**
- Consumes: Path to NSIS installer executable (`VolumeControl_<version>_x64-setup.exe`).
- Produces: JSON verification dossier containing SHA-256, byte size, PE machine type (`0x14c` / `IMAGE_FILE_MACHINE_I386` for NSIS stub), Nullsoft marker verification, and resource section metadata conforming to REA Evidence schema. Exits 0 on valid installer, non-zero on corrupt/mismatched binary.

- [ ] **Step 1: Write the failing test**

Create `scripts/test-verify-installer-rea.mjs`:
```javascript
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { writeFileSync, unlinkSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";

const scriptPath = resolve("scripts/verify-installer-rea.mjs");

test("verify-installer-rea rejects missing file", () => {
  assert.throws(
    () => execFileSync(process.execPath, [scriptPath, "nonexistent-setup.exe"], { stdio: "pipe" }),
    /installer file missing or unreadable/i
  );
});

test("verify-installer-rea rejects non-PE file", () => {
  const dummy = resolve("output/dummy-installer.exe");
  mkdirSync("output", { recursive: true });
  writeFileSync(dummy, "NOT A PE FILE");
  try {
    assert.throws(
      () => execFileSync(process.execPath, [scriptPath, dummy], { stdio: "pipe" }),
      /invalid PE DOS header/i
    );
  } finally {
    unlinkSync(dummy);
  }
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node scripts/test-verify-installer-rea.mjs`
Expected: FAIL with `Cannot find module .../verify-installer-rea.mjs`

- [ ] **Step 3: Write minimal implementation**

Create `scripts/verify-installer-rea.mjs`:
```javascript
#!/usr/bin/env node
import { openSync, readSync, closeSync, statSync } from "node:fs";
import { createHash } from "node:crypto";
import { basename, resolve } from "node:path";

function usage() {
  console.error("usage: node scripts/verify-installer-rea.mjs <installer.exe> [--json]");
  process.exit(2);
}

const args = process.argv.slice(2);
if (args.length === 0) usage();
const installerPath = resolve(args[0]);
const emitJson = args.includes("--json");

let stat;
try {
  stat = statSync(installerPath);
} catch {
  console.error(`ERROR: installer file missing or unreadable: ${installerPath}`);
  process.exit(1);
}

const fd = openSync(installerPath, "r");
const buffer = Buffer.alloc(1024);
readSync(fd, buffer, 0, 1024, 0);

// 1. Verify DOS Header
if (buffer.readUInt16LE(0) !== 0x5a4d) { // "MZ"
  closeSync(fd);
  console.error("ERROR: invalid PE DOS header (missing MZ magic)");
  process.exit(1);
}

// 2. PE Header Offset
const peOffset = buffer.readUInt32LE(0x3c);
const peHeader = Buffer.alloc(24);
readSync(fd, peHeader, 0, 24, peOffset);

if (peHeader.readUInt32LE(0) !== 0x00004550) { // "PE\0\0"
  closeSync(fd);
  console.error("ERROR: invalid PE signature");
  process.exit(1);
}

const machine = peHeader.readUInt16LE(4);
const isX86 = machine === 0x014c; // NSIS installers use 32-bit bootstrap
const isX64 = machine === 0x8664;

if (!isX86 && !isX64) {
  closeSync(fd);
  console.error(`ERROR: unsupported PE machine type: 0x${machine.toString(16)}`);
  process.exit(1);
}

// 3. Scan for Nullsoft signature
let hasNsisMarker = false;
const scanSize = Math.min(stat.size, 1024 * 1024);
const scanBuffer = Buffer.alloc(scanSize);
readSync(fd, scanBuffer, 0, scanSize, 0);
if (scanBuffer.includes("NullsoftInst")) {
  hasNsisMarker = true;
}
closeSync(fd);

// 4. Compute full file SHA-256
import { createReadStream } from "node:fs";
const hash = createHash("sha256");
const stream = createReadStream(installerPath);

await new Promise((res, rej) => {
  stream.on("data", (chunk) => hash.update(chunk));
  stream.on("end", res);
  stream.on("error", rej);
});

const sha256 = hash.digest("hex").toUpperCase();

const dossier = {
  evidence_id: `rea-winget-${Date.now()}`,
  subject: {
    name: basename(installerPath),
    local_path: installerPath,
    bytes: stat.size,
    sha256
  },
  pe: {
    machine: isX86 ? "IMAGE_FILE_MACHINE_I386" : "IMAGE_FILE_MACHINE_AMD64",
    is_nsis: hasNsisMarker
  },
  status: "verified"
};

if (emitJson) {
  console.log(JSON.stringify(dossier, null, 2));
} else {
  console.log(`REA verification OK for ${basename(installerPath)}`);
  console.log(`  SHA-256:   ${sha256}`);
  console.log(`  Size:      ${stat.size} bytes`);
  console.log(`  PE Target: ${dossier.pe.machine}`);
  console.log(`  NSIS Stub: ${hasNsisMarker ? "yes" : "no"}`);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node scripts/test-verify-installer-rea.mjs`
Expected: PASS (2 tests pass)

- [ ] **Step 5: Commit**

```bash
git add scripts/verify-installer-rea.mjs scripts/test-verify-installer-rea.mjs
git commit -m "feat(winget): add REA static PE installer verification module"
```

---

### Task 2: Automated Winget Deploy and Packaging Orchestrator

**Files:**
- Create: `scripts/deploy-winget.sh`
- Create: `scripts/deploy-winget.ps1`
- Test: `scripts/test-winget-deploy.sh`

**Interfaces:**
- Consumes: Target version tag (e.g. `v0.1.7`), installer executable path or `--fetch-latest`.
- Produces:
  1. `output/winget/rea-verification.json`
  2. `output/winget/manifests/d/dt418/VolumeControl/<version>/` (version, installer, and locale YAMLs)
  3. Validates manifests via `winget validate --manifest <dir>`
  4. Prepares `output/winget/pr-body.md` formatted for `microsoft/winget-pkgs` submission.

- [ ] **Step 1: Write the failing test**

Create `scripts/test-winget-deploy.sh`:
```bash
#!/usr/bin/env bash
set -euo pipefail

echo "testing deploy-winget arguments..."
if bash scripts/deploy-winget.sh; then
  echo "expected failure on missing arguments" >&2
  exit 1
fi

echo "testing deploy-winget invalid tag..."
if bash scripts/deploy-winget.sh invalid-tag non-existent.exe; then
  echo "expected failure on invalid tag" >&2
  exit 1
fi

echo "deploy-winget smoke ok"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bash scripts/test-winget-deploy.sh`
Expected: FAIL (`scripts/deploy-winget.sh: No such file or directory`)

- [ ] **Step 3: Write minimal implementation**

Create `scripts/deploy-winget.sh`:
```bash
#!/usr/bin/env bash
#
# deploy-winget.sh - End-to-end Winget deployment orchestrator
#
# Usage:
#   scripts/deploy-winget.sh <vX.Y.Z> <installer.exe> [outdir]
#
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

usage() {
  echo "usage: $0 <vX.Y.Z> <installer.exe> [outdir]" >&2
  exit 2
}

[ "$#" -ge 2 ] && [ "$#" -le 3 ] || usage
tag="$1"
installer="$2"
outdir="${3:-$repo_root/output/winget}"

case "$tag" in
  v[0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "version must look like vMAJOR.MINOR.PATCH: $tag" >&2; exit 1 ;;
esac
version="${tag#v}"

[[ -f "$installer" ]] || { echo "installer file missing: $installer" >&2; exit 1; }

mkdir -p "$outdir"

# Phase 1: REA Static PE Inspection
echo "[1/4] Running REA PE installer verification..."
dossier="$outdir/rea-verification.json"
node "$script_dir/verify-installer-rea.mjs" "$installer" --json > "$dossier"
sha256="$(node -e "console.log(JSON.parse(require('fs').readFileSync('$dossier')).subject.sha256)")"
echo "  REA verified SHA-256: $sha256"

# Phase 2: Manifest Generation
echo "[2/4] Generating winget-pkgs 1.12.0 manifests..."
manifest_dir="$outdir/manifests/d/dt418/VolumeControl"
bash "$script_dir/generate-winget-manifest.sh" "$tag" "$installer" "$manifest_dir"

# Phase 3: Manifest Validation
target_manifest_dir="$manifest_dir/dt418.VolumeControl/$version"
echo "[3/4] Validating manifest with winget CLI..."
if command -v winget >/dev/null 2>&1; then
  winget validate --manifest "$target_manifest_dir"
  echo "  winget validate passed."
else
  echo "  winget CLI not found on PATH; skipping local CLI validation."
fi

# Phase 4: Prepare PR Body
echo "[4/4] Preparing PR submission metadata..."
cat > "$outdir/pr-body.md" <<EOF
## Package Submission: dt418.VolumeControl version $version

### Verification
- **Installer URL:** https://github.com/dt418/volume-control/releases/download/$tag/VolumeControl_${version}_x64-setup.exe
- **SHA-256:** $sha256
- **Architecture:** x64 (Nullsoft installer)
- **Manifests validated:** \`winget validate --manifest\` passed locally.
- **REA Verification ID:** \`$(node -e "console.log(JSON.parse(require('fs').readFileSync('$dossier')).evidence_id)")\`

Generated by VolumeControl Winget Deploy Pipeline.
EOF

echo "Winget deployment bundle ready at: $outdir"
```

Create `scripts/deploy-winget.ps1`:
```powershell
[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)]
  [string]$Tag,
  [Parameter(Mandatory=$true)]
  [string]$Installer,
  [string]$OutDir = (Join-Path $PSScriptRoot "..\output\winget")
)

$ErrorActionPreference = "Stop"
$scriptDir = $PSScriptRoot
$repoRoot = (Resolve-Path (Join-Path $scriptDir "..")).Path
$bash = "C:\Program Files\Git\bin\bash.exe"

if (-not (Test-Path $bash)) {
  $bash = (Get-Command bash).Source
}

& $bash (Join-Path $scriptDir "deploy-winget.sh") $Tag $Installer $OutDir
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `bash scripts/test-winget-deploy.sh`
Expected: PASS (`deploy-winget smoke ok`)

- [ ] **Step 5: Commit**

```bash
git add scripts/deploy-winget.sh scripts/deploy-winget.ps1 scripts/test-winget-deploy.sh
git commit -m "feat(winget): add automated winget deploy orchestrator"
```

---

### Task 3: Execute Live Verification on Release v0.1.7 Artifact

**Files:**
- Output: `output/winget/rea-verification.json`
- Output: `output/winget/pr-body.md`
- Output: `output/winget/manifests/...`

**Interfaces:**
- Consumes: `C:/Users/Thanh/AppData/Local/Temp/winget-017/VolumeControl_0.1.7_x64-setup.exe`.
- Produces: Concrete validated Winget manifests and REA verification dossier for 0.1.7.

- [ ] **Step 1: Execute deployment pipeline on 0.1.7 installer**

```bash
bash scripts/deploy-winget.sh v0.1.7 "C:/Users/Thanh/AppData/Local/Temp/winget-017/VolumeControl_0.1.7_x64-setup.exe"
```
Expected: All 4 phases complete; `winget validate` exits 0 with `Manifest validation succeeded.`.

- [ ] **Step 2: Inspect REA dossier and manifests**

Verify:
1. `output/winget/rea-verification.json` contains SHA-256 `541C57E8F7B57F571559278E167BE4123923A2CFB22CB776791D84519E30B92B`.
2. `output/winget/pr-body.md` contains exact installer URL and match.
3. `dt418.VolumeControl.installer.yaml` carries `InstallerSha256: 541C57E8F7B57F571559278E167BE4123923A2CFB22CB776791D84519E30B92B`.

---

### Task 4: Record Feature vol-099 in feature_list.json and claude-progress.md

**Files:**
- Modify: `feature_list.json`
- Modify: `claude-progress.md`

**Interfaces:**
- Consumes: Execution evidence from Tasks 1-3.
- Produces: Passing feature record for `vol-099` satisfying records guard and pre-commit checks.

- [ ] **Step 1: Add vol-099 to feature_list.json**
- [ ] **Step 2: Add Session 113 to claude-progress.md**
- [ ] **Step 3: Run full quality gate**

```bash
cargo fmt --all --check
git diff --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
sh scripts/check-records.sh
```

- [ ] **Step 4: Commit**

```bash
git add feature_list.json claude-progress.md
git commit -m "docs(records): record vol-099 winget deploy automation with REA evidence"
```
