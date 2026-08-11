# windows-host Skill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Package the seven Windows-host environment/tooling gotchas hit in Session 034 into a `windows-host` skill with a pkg-config stub helper script, wired into the records-guard self-test.

**Architecture:** New skill directory `.agents/skills/windows-host/` (SKILL.md + scripts/ensure-pkg-config-stub.ps1) mirrored byte-identical to `.claude/skills/windows-host/`; one new mirror-identity assertion appended to `scripts/test-check-records.sh` (32 -> 33 checks); baseline count text bumped in pre-push-review SKILL.md (both mirrors) and session-handoff.md.

**Tech Stack:** PowerShell 5.1, POSIX sh, bash (Git Bash), git, cargo.

**Design spec:** `docs/superpowers/specs/2026-08-12-windows-host-skill-design.md` (committed 6a99cc2).

## Global Constraints

- Every substantive change set must update BOTH `feature_list.json` and `claude-progress.md` in the same commit (guardrail rule; enforced by pre-commit hook `check-records.sh --staged`). `docs/**` is exempt.
- Skill mirrors (`.agents` <-> `.claude`) must be byte-identical; `test-check-records.sh` asserts this for guardrail/pre-push-review and will assert it for windows-host.
- Never write tracked markdown/JSON with PowerShell `Set-Content -Encoding UTF8` (BOM + em-dash double-encode). Use the edit tool or `[System.IO.File]::WriteAllText` with `UTF8Encoding($false)`.
- Self-tests run under Git Bash (`bash scripts/...`), never `sh` from PowerShell and never the WSL shim.
- After this change set lands, `test-check-records.sh` runs 33 checks; pre-push-review SKILL.md baseline and session-handoff.md must say 33.
- Records land in the same commit as code. Ship via `scripts/ship.ps1 -Push`.

---

### Task 1: Helper script ensure-pkg-config-stub.ps1 (both mirrors)

**Files:**
- Create: `.agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1`
- Create: `.claude/skills/windows-host/scripts/ensure-pkg-config-stub.ps1` (byte-identical copy)

**Interfaces:**
- Consumes: nothing (standalone)
- Produces: `%TEMP%\rtk-stub-bin\pkg-config.cmd` (the stub), exit 0 when the stub answers `--modversion` with `1.0`, exit 1 otherwise; prints stub path + env vars to set.

- [ ] **Step 1: Create the script in `.agents`**

```powershell
<#
.SYNOPSIS
    Ensure the pkg-config cross-check stub exists and works.
.DESCRIPTION
    Environment shim for cross-target cargo checks from a Windows host
    (libpulse-sys's build script needs pkg-config). Creates
    %TEMP%\rtk-stub-bin\pkg-config.cmd if missing and verifies it answers
    --modversion. Writes UTF-8 no-BOM (never Set-Content).
.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1
#>

$ErrorActionPreference = 'Stop'

$stubDir = Join-Path $env:TEMP 'rtk-stub-bin'
$stubPath = Join-Path $stubDir 'pkg-config.cmd'
# A pure exit-0 stub panics inside pkg-config-0.3.33 parse_modversion
# (Option::unwrap on empty --modversion output), so it must print a version.
$stubContent = "@echo off`r`nfor %%a in (%*) do if ""%%a""==""--modversion"" echo 1.0`r`nexit /b 0`r`n"

if (-not (Test-Path -LiteralPath $stubDir)) {
    New-Item -ItemType Directory -Path $stubDir | Out-Null
}

$needWrite = $true
if (Test-Path -LiteralPath $stubPath) {
    $existing = [System.IO.File]::ReadAllText($stubPath)
    if ($existing -eq $stubContent) {
        $needWrite = $false
    } else {
        Write-Host "Existing stub differs; overwriting: $stubPath"
    }
}

if ($needWrite) {
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($stubPath, $stubContent, $utf8NoBom)
    Write-Host "Created stub: $stubPath"
}

$out = & $stubPath --modversion 2>$null
if ($LASTEXITCODE -ne 0 -or "$out".Trim() -ne '1.0') {
    Write-Host "ERROR: stub does not answer --modversion correctly" -ForegroundColor Red
    exit 1
}

Write-Host "Stub ready: $stubPath"
Write-Host "Set for cross-target checks:"
Write-Host "  `$env:PKG_CONFIG = `"$stubPath`""
Write-Host "  `$env:PKG_CONFIG_ALLOW_CROSS = '1'"
exit 0
```

- [ ] **Step 2: Copy to `.claude` and verify byte-identical**

```powershell
New-Item -ItemType Directory -Path '.claude\skills\windows-host\scripts' -Force | Out-Null
Copy-Item -LiteralPath '.agents\skills\windows-host\scripts\ensure-pkg-config-stub.ps1' -Destination '.claude\skills\windows-host\scripts\ensure-pkg-config-stub.ps1' -Force
(Get-FileHash '.agents\skills\windows-host\scripts\ensure-pkg-config-stub.ps1' -Algorithm SHA256).Hash -eq (Get-FileHash '.claude\skills\windows-host\scripts\ensure-pkg-config-stub.ps1' -Algorithm SHA256).Hash
```

Expected: `True`.

- [ ] **Step 3: Delete the old stub, run the script, verify creation + idempotency**

```powershell
Remove-Item -LiteralPath "$env:TEMP\rtk-stub-bin\pkg-config.cmd" -Force -ErrorAction SilentlyContinue
powershell -ExecutionPolicy Bypass -File .agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1
powershell -ExecutionPolicy Bypass -File .agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1
```

Expected: first run prints "Created stub", second run does not overwrite; both exit 0, both print "Stub ready" and the two env vars.

- [ ] **Step 4: Sanity-check the stub against a real cross-check**

```powershell
$env:PKG_CONFIG = "$env:TEMP\rtk-stub-bin\pkg-config.cmd"
$env:PKG_CONFIG_ALLOW_CROSS = '1'
cargo check --target x86_64-unknown-linux-gnu -p volumectl --tests --no-default-features --features gtk-renderer
```

Expected: `Finished` clean (no pkg-config cross-compilation error).

- [ ] **Step 5: Commit**

```bash
git add .agents/skills/windows-host .claude/skills/windows-host
git commit -m "feat(tooling): add windows-host pkg-config stub helper"
```

Note: this commit will fail the records guard if it is the only staged change (substantive). If committing standalone, stage the records updates from Task 4 alongside; otherwise defer this commit until Task 4 and commit once at the end. Recommend: commit once at the end with records.

---

### Task 2: SKILL.md (both mirrors)

**Files:**
- Create: `.agents/skills/windows-host/SKILL.md`
- Create: `.claude/skills/windows-host/SKILL.md` (byte-identical copy)

**Interfaces:**
- Consumes: Task 1's `scripts/ensure-pkg-config-stub.ps1` (referenced from gotcha #2)
- Produces: the seven-gotcha reference every future session reads.

- [ ] **Step 1: Write SKILL.md in `.agents`** (use the edit/write tool — no Set-Content)

Content (frontmatter `name: windows-host`; section per gotcha, each with Trigger -> Symptom -> Fix; a "Verification commands" section; a "Before commit" checklist):

```markdown
---
name: windows-host
description: Windows-host development environment for volume-control — encoding-safe file edits, pkg-config cross-check stub, Git Bash vs sh, skill mirror sync, stale-tree hygiene, handoff refresh, safe shell invocation. Use when working in this repo from a Windows host, before cross-target cargo checks, self-tests, or any file edit on tracked markdown/JSON.
---

# Windows-host development (volume-control)

Seven environment/tooling gotchas, each with trigger -> symptom -> fix.
These cost real time in Session 034; follow them to avoid repeating.

## 1. Encoding-safe edits (never Set-Content on tracked text files)

- Trigger: editing `claude-progress.md`, `feature_list.json`, any tracked .md/.json.
- Symptom: PowerShell `Set-Content -Encoding UTF8` adds a BOM and double-encodes
  em-dashes (`—` becomes `â€”`), rewriting the whole file as giant diff churn.
- Fix: use the edit tool for tracked docs. For generated files use
  `[System.IO.File]::WriteAllText($path, $content, New-Object System.Text.UTF8Encoding($false))`.
  After any PowerShell write, verify: `git diff --stat <file>` shows minimal
  changes, and the first 3 bytes are not EF BB BF.

## 2. pkg-config cross-check stub

- Trigger: cross-target checks from Windows
  (`cargo check --target x86_64-unknown-linux-gnu|apple-darwin -p volumectl --tests`).
- Symptom: `pkg-config has not been configured to support cross-compilation`
  (libpulse-sys 1.23.0 build script) when the stub is missing.
- Fix: `powershell -ExecutionPolicy Bypass -File
  .agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1`, then set:
  `$env:PKG_CONFIG = "$env:TEMP\rtk-stub-bin\pkg-config.cmd"`,
  `$env:PKG_CONFIG_ALLOW_CROSS = '1'`.
- Stub is an environment shim, not a repo artifact; it gets deleted — recreate
  with the helper. The stub MUST print a version for `--modversion` (a pure
  exit-0 stub panics inside pkg-config-0.3.33 `parse_modversion`,
  Option::unwrap on empty output).

## 3. Git Bash, not sh, not WSL shim

- Trigger: running `scripts/*.sh`, self-tests, ship flow from PowerShell.
- Symptom: `sh scripts/test-check-records.sh` -> 9 spurious FAILs
  (`fatal: cannot change to '/tmp/...'`) because MSYS `/tmp` paths break native
  git.exe. The WSL shim (`System32\bash.exe`) silently skips PowerShell checks.
- Fix: always `bash scripts/...` (Git for Windows bash), e.g.
  `bash scripts/test-check-records.sh`, `bash scripts/format-lint.sh`,
  `bash scripts/ship.sh --dry-run`.

## 4. Skill mirror sync

- Trigger: editing `scripts/*.sh` or `scripts/*.ps1` that a skill mirrors
  (format-lint, guardrail, pre-push-review, windows-host).
- Symptom: `bash scripts/test-format-lint.sh` FAILs
  ("mirrors: format-lint.sh copies differ").
- Fix: copy the edited file to BOTH
  `.agents/skills/<skill>/scripts/` and `.claude/skills/<skill>/scripts/`
  byte-identical, then re-run the smoke test. Same rule for new skills:
  every skill exists in both trees.

## 5. Stale-tree hygiene before commit

- Trigger: about to `git add -A` or commit.
- Symptom: third skill trees and unwired lock files rot unnoticed
  (history: `agent/skills/`, `skills-lock.json` were stale zero-reference
  artifacts, both deleted).
- Fix: `git status --short` — if you see a tree/lock file you did not touch,
  grep the repo for references before keeping it; delete if unwired
  (recoverable in git).

## 6. Session handoff refresh

- Trigger: after every substantive landing.
- Symptom: session-handoff.md shows an older session number, feature count, or
  test count, and the next session trusts the stale numbers.
- Fix: update `session-handoff.md` — session number, feature count, unit-test
  count, enforcement self-test counts (records/format-lint/ship). Stale counts
  there are how sessions 022-033 drifted.

## 7. Safe shell invocation from PowerShell

- Trigger: calling `bash`/`sh -c` with nested quotes from PowerShell.
- Symptom: quote-escaping breaks (`MissingEqualsInHashLiteral`, mangled args).
- Fix: write a temp script under `C:\Users\<user>\AppData\Local\Temp\opencode\`
  and run it, instead of inlining nested quoting.

## Verification commands (Windows host)

```
# Cross-target checks (after ensure-pkg-config-stub.ps1):
$env:PKG_CONFIG = "$env:TEMP\rtk-stub-bin\pkg-config.cmd"; $env:PKG_CONFIG_ALLOW_CROSS = '1'
cargo check --target x86_64-unknown-linux-gnu -p volumectl --tests --no-default-features --features gtk-renderer
cargo check --target x86_64-apple-darwin -p volumectl --tests --no-default-features

# Self-tests (Git Bash, not sh / WSL shim):
bash scripts/test-check-records.sh
bash scripts/test-format-lint.sh
bash scripts/test-ship.sh

# Records guard + ship:
sh scripts/check-records.sh --branch origin/master
powershell -ExecutionPolicy Bypass -File scripts/ship.ps1 -Push
```

## Before commit checklist

- [ ] No PowerShell Set-Content on tracked .md/.json this session
- [ ] Skill mirrors byte-identical (`cmp -s` or hash check)
- [ ] Self-tests run under bash, exit 0
- [ ] session-handoff.md refreshed
- [ ] feature_list.json + claude-progress.md updated in the same commit
```

- [ ] **Step 2: Copy to `.claude` and verify byte-identical**

```powershell
Copy-Item -LiteralPath '.agents\skills\windows-host\SKILL.md' -Destination '.claude\skills\windows-host\SKILL.md' -Force
(Get-FileHash '.agents\skills\windows-host\SKILL.md' -Algorithm SHA256).Hash -eq (Get-FileHash '.claude\skills\windows-host\SKILL.md' -Algorithm SHA256).Hash
```

Expected: `True`.

- [ ] **Step 3: Commit** (or defer to Task 4's single commit — same recommendation as Task 1 Step 5)

```bash
git add .agents/skills/windows-host .claude/skills/windows-host
git commit -m "feat(tooling): add windows-host gotchas skill"
```

---

### Task 3: Wire mirror assertion into test-check-records.sh + bump baselines

**Files:**
- Modify: `scripts/test-check-records.sh` (append mirror check after line 350, before the failures counter)
- Modify: `.agents/skills/pre-push-review/SKILL.md` (baseline "records 32" -> 33)
- Modify: `.claude/skills/pre-push-review/SKILL.md` (same, byte-identical)
- Modify: `session-handoff.md` (self-test counts table: records 32 -> 33)
- Modify: `feature_list.json` (vol-017 verification line "32 checks" -> 33; new vol-027 entry in Task 4)

**Interfaces:**
- Consumes: Task 1+2 skill files (asserted paths)
- Produces: 33-check suite; the count every baseline text references.

- [ ] **Step 1: Append the mirror check** — insert after the guardrail block (line 350), before `if [ "$failures" -eq 0 ]`:

```sh
if [ -f .agents/skills/windows-host/SKILL.md ] && \
   [ -f .claude/skills/windows-host/SKILL.md ] && \
   cmp -s .agents/skills/windows-host/SKILL.md .claude/skills/windows-host/SKILL.md && \
   cmp -s .agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1 \
         .claude/skills/windows-host/scripts/ensure-pkg-config-stub.ps1; then
    report ok 'windows-host skill: .agents/.claude mirrors are byte-identical'
else
    report FAIL 'windows-host skill: .agents/.claude mirrors differ (resync .claude/skills/windows-host/)'
fi
```

- [ ] **Step 2: Bump the check count in both pre-push-review mirrors**

`.agents/skills/pre-push-review/SKILL.md` line ~104: `(current baseline: records 32, format-lint 39 on Windows / 38 on Linux/macOS, ship 22; ...` -> `records 33, ...`. Then `Copy-Item` to `.claude/skills/pre-push-review/SKILL.md`, hash-verify identical.

- [ ] **Step 3: Bump session-handoff.md counts table** — `| test-check-records.sh | 32 | Windows; Linux/macOS same |` -> `33`.

- [ ] **Step 4: Update vol-017 verification line** — `feature_list.json` line ~155 "32 checks incl." -> "33 checks incl." (use the edit tool; the file's `verification` array is the CURRENT expectation).

- [ ] **Step 5: Run the battery**

```powershell
bash scripts/test-check-records.sh
```

Expected: 33 checks, "All record-keeping guard checks passed.", exit 0. Then `bash scripts/test-format-lint.sh`, `bash scripts/test-ship.sh`, both gates (`bash scripts/format-lint.sh`, `powershell -ExecutionPolicy Bypass -File .agents/skills/format-lint/scripts/format-lint.ps1`) — all exit 0.

- [ ] **Step 6: Commit** (or defer to Task 4 — recommend deferring)

```bash
git add scripts/test-check-records.sh .agents/skills/pre-push-review .claude/skills/pre-push-review session-handoff.md feature_list.json
git commit -m "chore(tooling): assert windows-host mirror identity in records self-test"
```

---

### Task 4: Records (vol-027 + Session 035) + ship

**Files:**
- Modify: `feature_list.json` (new vol-027 entry at the top of `features`, `last_updated` bump)
- Modify: `claude-progress.md` (Session 035 entry at the top)
- All Task 1-3 files land in this commit

**Interfaces:**
- Consumes: the completed skill + wiring
- Produces: the record trail the guard requires.

- [ ] **Step 1: Add vol-027 to feature_list.json** (edit tool; insert at the top of `features` array)

```json
{
    "id": "vol-027",
    "priority": 27,
    "area": "tooling",
    "title": "windows-host skill: environment gotchas + pkg-config stub helper + mirror wiring",
    "user_visible_behavior": "Developer/agent-facing: future Windows-hosted sessions follow seven documented gotchas (encoding-safe edits, pkg-config stub, Git Bash vs sh, mirror sync, stale-tree hygiene, handoff refresh, safe shell invocation) and can recreate the pkg-config cross-check stub with one script; a windows-host mirror drift now fails test-check-records.sh.",
    "status": "passing",
    "verification": [
        "Run `powershell -ExecutionPolicy Bypass -File .agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1` twice - first creates the stub, second is idempotent; both exit 0.",
        "Run `bash scripts/test-check-records.sh` - 33 checks pass, exit 0 (incl. windows-host mirror byte-identity).",
        "Run `bash scripts/test-format-lint.sh` and `bash scripts/test-ship.sh` - all pass, exit 0.",
        "Run both full format-lint gates (bash + PowerShell) with tests - 'Gate passed.' on both.",
        "Cross-check sanity: `cargo check --target x86_64-unknown-linux-gnu -p volumectl --tests --no-default-features --features gtk-renderer` with stub env - compiles clean.",
        "Run `sh scripts/check-records.sh --branch origin/master` and `--staged` on the landed set - exit 0."
    ],
    "evidence": [
        "Session 035 (2026-08-12): skill lands in both mirrors byte-identical; helper recreates the stub after deletion and answers --modversion with 1.0; test-check-records.sh grew 32 -> 33 checks; pre-push-review baseline and session-handoff.md bumped to 33; design spec docs/superpowers/specs/2026-08-12-windows-host-skill-design.md."
    ],
    "notes": "Packages the Session 034 gotchas (encoding corruption, lost stub, sh-vs-bash, mirror drift, stale trees, handoff staleness, nested quoting) as a skill so future sessions do not repeat them."
}
```

Also bump `last_updated` to `2026-08-12T<HH:MM>`.

- [ ] **Step 2: Add Session 035 to claude-progress.md** (edit tool, top of file, after `# Progress Log`)

```markdown
## Session 035 (2026-08-12) — windows-host skill: environment gotchas + stub helper + wiring

- Goal: package the seven Windows-host tooling gotchas from Session 034 as a
  skill so future sessions do not repeat them; ship a helper that recreates
  the pkg-config cross-check stub; wire the mirror into the records self-test.
- What landed:
  - `.agents/skills/windows-host/SKILL.md` + `.claude/...` (byte-identical):
    seven gotchas with trigger/symptom/fix — encoding-safe edits (no
    Set-Content on tracked text), pkg-config stub recreation, Git Bash vs sh,
    skill mirror sync, stale-tree hygiene, session-handoff refresh, safe
    shell invocation from PowerShell.
  - `.agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1` +
    `.claude/...`: idempotent stub creator/verifier; writes UTF-8 no-BOM;
    verifies `--modversion` answers 1.0; prints the env vars to set.
  - `scripts/test-check-records.sh`: windows-host mirror byte-identity
    assertion (32 -> 33 checks).
  - Baselines bumped: pre-push-review SKILL.md (both mirrors) and
    session-handoff.md say records 33; feature_list vol-017 verification line
    updated to 33.
  - `feature_list.json`: vol-027 entry, `last_updated` bumped.
- Verification:
  - Helper run twice: first "Created stub", second idempotent, both exit 0.
  - `bash scripts/test-check-records.sh` - 33 checks pass.
  - `bash scripts/test-format-lint.sh`, `bash scripts/test-ship.sh`, both
    gates with tests - all green.
  - Linux cross-check with stub env - compiles clean.
  - `sh scripts/check-records.sh --branch origin/master` + `--staged` - exit 0.
  - Shipped via `scripts/ship.ps1 -Push`.
```

- [ ] **Step 3: Stage everything + verify the staged guard**

```powershell
git add -A
sh scripts/check-records.sh --staged
```

Expected: exit 0.

- [ ] **Step 4: Ship**

```powershell
powershell -ExecutionPolicy Bypass -File scripts/ship.ps1 -Push
```

Expected: all 5 ship checks pass, commit created, `master -> master` push line, exit 0.

- [ ] **Step 5: Post-ship sanity**

```powershell
git log --oneline -2
git status --short | Measure-Object | Select-Object -ExpandProperty Count
```

Expected: 2 recent commits (design spec + skill commit), 0 dirty files.
