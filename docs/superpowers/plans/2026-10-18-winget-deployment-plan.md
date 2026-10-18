# Winget Deployment Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement automated REA static PE verification, generate and validate Winget 1.12.0 manifests for VolumeControl v0.1.7, verify local installation, track upstream PR #454508 status, and record complete verification evidence.

**Architecture:** A Node.js static binary parser (`scripts/verify-installer-rea.mjs`) reads PE headers, NSIS signatures, elevation level, switches, and SHA-256 without execution; `scripts/generate-winget-manifest.sh` produces schema-valid 1.12.0 manifests; `scripts/test-winget-deploy.sh` verifies negative and positive cases; `winget` CLI validates and installs locally; records update enforces the repository guardrail.

**Tech Stack:** Node.js (Buffer / crypto / PE parsing), Bash, Winget CLI, GitHub CLI (`gh`), Git.

## Global Constraints

- Installer filename: `VolumeControl_0.1.7_x64-setup.exe`
- Target release: `v0.1.7`
- Expected SHA-256: `541C57E8F7B57F571559278E167BE4123923A2CFB22CB776791D84519E30B92B`
- Winget Manifest version: `1.12.0`
- Manifest package ID: `dt418.VolumeControl`
- Guardrail: Update `feature_list.json` (`vol-099`) and `claude-progress.md` in the substantive changeset.

---

### Task 1: Acquire Artifact and Implement REA Static PE Verifier

**Files:**
- Create: `scripts/verify-installer-rea.mjs`
- Test: `node scripts/verify-installer-rea.mjs VolumeControl_0.1.7_x64-setup.exe`

**Interfaces:**
- Consumes: Released binary `VolumeControl_0.1.7_x64-setup.exe`
- Produces: JSON summary / exit code 0 on valid NSIS x64 installer matching criteria:
  - MZ DOS header + PE\0\0 signature
  - COFF machine: `0x014c` (i386 stub)
  - NSIS marker: `NullsoftInst`
  - Elevation level: `asInvoker`
  - Switches: `/S` and `/D`
  - Registry uninstaller path: `Software\Microsoft\Windows\CurrentVersion\Uninstall\VolumeControl`
  - SHA-256 uppercase hash

- [ ] **Step 1: Download official artifact if not present**
```bash
curl -fL -o VolumeControl_0.1.7_x64-setup.exe https://github.com/dt418/volume-control/releases/download/v0.1.7/VolumeControl_0.1.7_x64-setup.exe
```

- [ ] **Step 2: Implement scripts/verify-installer-rea.mjs**
Write `scripts/verify-installer-rea.mjs` with full PE / NSIS parsing and assertions.

- [ ] **Step 3: Run verifier against official binary**
```bash
node scripts/verify-installer-rea.mjs VolumeControl_0.1.7_x64-setup.exe
```
Expected: Exit code 0, all binary checks passed, SHA-256 matches `541C57E8F7B57F571559278E167BE4123923A2CFB22CB776791D84519E30B92B`.

---

### Task 2: Implement Automated Deployment Test Suite

**Files:**
- Create: `scripts/test-winget-deploy.sh`
- Test: `bash scripts/test-winget-deploy.sh`

**Interfaces:**
- Consumes: `scripts/verify-installer-rea.mjs`, `scripts/generate-winget-manifest.sh`
- Produces: Exit code 0 on all test cases (rejects non-existent file, rejects non-PE, rejects wrong PE, validates manifest generation).

- [ ] **Step 1: Write scripts/test-winget-deploy.sh**
Implement unit/integration checks covering positive and negative cases.

- [ ] **Step 2: Run test suite**
```bash
bash scripts/test-winget-deploy.sh
```
Expected: All tests pass.

---

### Task 3: Generate and Validate Winget Manifests

**Files:**
- Create: `winget/dt418.VolumeControl/0.1.7/dt418.VolumeControl.yaml`
- Create: `winget/dt418.VolumeControl/0.1.7/dt418.VolumeControl.installer.yaml`
- Create: `winget/dt418.VolumeControl/0.1.7/dt418.VolumeControl.locale.en-US.yaml`

**Interfaces:**
- Consumes: `scripts/generate-winget-manifest.sh`, `VolumeControl_0.1.7_x64-setup.exe`
- Produces: Validated YAML trio in `winget/`

- [ ] **Step 1: Run generate script**
```bash
bash scripts/generate-winget-manifest.sh v0.1.7 VolumeControl_0.1.7_x64-setup.exe winget
```

- [ ] **Step 2: Validate with winget CLI**
```bash
winget validate --manifest winget/dt418.VolumeControl/0.1.7
```
Expected: Manifest validation successful.

- [ ] **Step 3: Local installation test**
```bash
winget install --manifest winget/dt418.VolumeControl/0.1.7 --accept-package-agreements --accept-source-agreements
```
Expected: Installs locally or verifies execution path.

---

### Task 4: Audit Upstream PR #454508 and Update Documentation

**Files:**
- Read/Check: `gh pr view 454508 --repo microsoft/winget-pkgs`
- Modify: `docs/index.html` (if applicable / comments on upstream state)

**Interfaces:**
- Consumes: GitHub API / Winget repository PR 454508 status
- Produces: Verified upstream status log and accurate landing page configuration.

- [ ] **Step 1: Check upstream PR state and checks**
Run `gh pr view 454508 --repo microsoft/winget-pkgs` and check review status.

- [ ] **Step 2: Test winget search in official repo**
```bash
winget search dt418.VolumeControl
```
Document whether package has landed in public CDN index or is waiting for moderator approval.

- [ ] **Step 3: Update docs/index.html documentation notes**
Keep `var WINGET_LIVE = false` (since PR is pending moderator merge), ensure inline comments describe exact conditions when PR #454508 merges.

---

### Task 5: Update Records and Quality Gate

**Files:**
- Modify: `feature_list.json` (`vol-099` status -> passing, add full verification & evidence)
- Modify: `claude-progress.md` (add Session 116 entry)

**Interfaces:**
- Consumes: Verification output from Tasks 1-4
- Produces: Guardrail compliance

- [ ] **Step 1: Update feature_list.json**
Mark `vol-099` as `passing` and populate verification commands and evidence.

- [ ] **Step 2: Update claude-progress.md**
Add Session 116 log with exact REA findings, SHA-256, winget validate output, and PR 454508 state.

- [ ] **Step 3: Run repository quality gate**
```bash
bash scripts/check-records.sh --staged
```
Expected: Pass without warnings.
