# Format-Lint Gate Toolchain Landing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the manifest-driven format-lint gate toolchain in two logical commits and wire its smoke test into CI on the ubuntu `checks` and `windows` jobs.

**Architecture:** The toolchain already exists on disk and is verified working; this plan commits it. `scripts/format-lint-steps.json` is the single source of truth (5 steps + 6 forbidden-diff-path patterns) that the bash gate (`scripts/format-lint.sh`) and the PowerShell gate (`.agents/skills/format-lint/scripts/format-lint.ps1`) both read and execute. `scripts/test-format-lint.sh` asserts both gates (24 checks). The CI wiring adds one smoke-test step to each of the `checks` (ubuntu) and `windows` jobs of `.github/workflows/ci.yml`.

**Tech Stack:** bash, PowerShell 5.1+, GitHub Actions (YAML), git, cargo/rustfmt/clippy, JSON manifest.

## Global Constraints

- Manifest `scripts/format-lint-steps.json` is version `2`; both gates reject any other version loudly.
- Manifest layout: one step object and one `forbidden_patterns` entry per line at 4-space indent; JSON values free of embedded quotes; `args` entries contain no spaces or glob characters.
- The smoke test must stay green at 24/24: `bash scripts/test-format-lint.sh` exits 0 with "All format-lint smoke checks passed."
- Mirror byte-identity must hold: `cmp -s` on `format-lint.ps1`, `format-lint.sh`, and `SKILL.md` between `.agents/skills/format-lint/` and `.claude/skills/format-lint/`.
- `.claude/settings.local.json` is tracked; it must remain byte-identical after every smoke-test run (the smoke test's EXIT trap restores it).
- No Rust source changes; no `.githooks/pre-commit` changes.
- Stage files by explicit path — never `git add -A` or `git add .`.
- Windows-gated CI step must set `shell: bash` (windows job steps default to pwsh).
- Commit messages follow the repo's conventional style: `feat:`, `ci:`, `docs:`.

---

### Task 1: Commit the gate toolchain (Commit 1)

**Files:**
- Stage: `.gitignore`, `Cargo.lock`, `scripts/format-lint-steps.json`, `scripts/format-lint.sh`, `scripts/test-format-lint.sh`, `.agents/skills/format-lint/`, `.claude/skills/format-lint/`, `docs/superpowers/specs/2026-08-08-format-lint-gate-toolchain-design.md`, `docs/superpowers/plans/2026-08-08-format-lint-gate-toolchain.md`
- Do NOT stage: `.github/workflows/ci.yml` (Task 2), `claude-progress.md` (Task 3)
- Test: `scripts/test-format-lint.sh`, both gates

**Interfaces:**
- Consumes: the files already on disk (baseline verified in this task)
- Produces: commit `feat: add manifest-driven format-lint gate toolchain with cross-platform smoke test`; a clean `git status` showing only `.github/workflows/ci.yml` as untouched; the verified baseline that Task 2 builds on

- [ ] **Step 1: Verify the baseline**

Run from the repository root:

```bash
bash scripts/test-format-lint.sh
```

Expected: `All format-lint smoke checks passed.` and exit code 0 (24 checks).

- [ ] **Step 2: Verify both full gates**

```bash
bash scripts/format-lint.sh
powershell -NoProfile -ExecutionPolicy Bypass -File .agents/skills/format-lint/scripts/format-lint.ps1
```

Expected: both print `Gate passed.` and exit 0.

- [ ] **Step 3: Verify mirror byte-identity and manifest version**

```bash
cmp -s .agents/skills/format-lint/scripts/format-lint.ps1 .claude/skills/format-lint/scripts/format-lint.ps1
cmp -s scripts/format-lint.sh .claude/skills/format-lint/scripts/format-lint.sh
cmp -s .agents/skills/format-lint/SKILL.md .claude/skills/format-lint/SKILL.md
grep -c '"version": 2' scripts/format-lint-steps.json
git ls-files | grep -x '.claude/settings.local.json'
git diff --quiet HEAD -- .claude/settings.local.json && echo clean
```

Expected: all three `cmp` exit 0; version count 1; the file is tracked; `clean`.

- [ ] **Step 4: Review the working tree before staging**

```bash
git status --short
```

Expected: exactly ` M .gitignore` plus the untracked entries listed in **Files** (and the two new docs). If anything unexpected appears, stop and investigate before staging.

- [ ] **Step 5: Stage the exact file list**

```bash
git add .gitignore Cargo.lock scripts/format-lint-steps.json scripts/format-lint.sh scripts/test-format-lint.sh .agents/skills/format-lint .claude/skills/format-lint docs/superpowers/specs/2026-08-08-format-lint-gate-toolchain-design.md docs/superpowers/plans/2026-08-08-format-lint-gate-toolchain.md
```

- [ ] **Step 6: Review the staged diff**

```bash
git diff --cached --stat
git status --short
```

Expected: the stat lists only the intended files; `git status` shows no unstaged changes except `.github/workflows/ci.yml` and `claude-progress.md` (both untouched).

- [ ] **Step 7: Commit**

```bash
git commit -m "feat: add manifest-driven format-lint gate toolchain with cross-platform smoke test"
```

- [ ] **Step 8: Verify the commit**

```bash
git status --short
git log --oneline -1
bash scripts/test-format-lint.sh
```

Expected: `git status` clean except `.github/workflows/ci.yml` (untouched, will be committed in Task 2); the new commit is HEAD; the smoke test still passes 24/24.

---

### Task 2: Wire the smoke test into CI (Commit 2)

**Files:**
- Modify: `.github/workflows/ci.yml` — two insertions (step added after `Test (no GTK features)` in the `checks` job; step added after `Test` in the `windows` job)
- Test: YAML validation + local smoke test re-run

**Interfaces:**
- Consumes: the committed `scripts/test-format-lint.sh` and both gates from Task 1 (they run from a fresh CI checkout)
- Produces: commit `ci: run format-lint smoke test on ubuntu checks and windows jobs`

- [ ] **Step 1: Locate the insertion points**

```bash
grep -n -A1 "name: Test" .github/workflows/ci.yml
```

Expected: several matches — the insertion points are the `Test (no GTK features)` step in the `checks` job and the `Test` step in the `windows` job (the macOS and ubuntu jobs' `Test` steps are not touched). Confirm each selected step is immediately followed by its `run:` line, then insert the new step after that `run:` line at 6-space indentation.

- [ ] **Step 2: Add the checks-job step**

Insert immediately after the `Test (no GTK features)` step (after line 43, at the same 6-space `- name:` indentation):

```yaml
      - name: Smoke-test format-lint gates
        run: bash scripts/test-format-lint.sh
```

- [ ] **Step 3: Add the windows-job step**

Insert immediately after the `Test` step (after line 55), with `shell: bash` because windows job steps default to pwsh:

```yaml
      - name: Smoke-test format-lint gates
        shell: bash
        run: bash scripts/test-format-lint.sh
```

- [ ] **Step 4: Validate the diff and YAML**

```bash
git diff .github/workflows/ci.yml
git diff --check .github/workflows/ci.yml
python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml')); print('yaml OK')" 2>/dev/null || echo "python3/pyyaml unavailable; rely on the diff review above and CI itself"
```

Expected: the diff shows exactly the two new steps at correct indentation, no whitespace errors; the YAML parse succeeds if pyyaml is available.

- [ ] **Step 5: Re-run the smoke test locally**

```bash
bash scripts/test-format-lint.sh
```

Expected: 24/24 pass (the CI change does not affect local gates, but confirms the tree is still green before committing).

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: run format-lint smoke test on ubuntu checks and windows jobs"
```

- [ ] **Step 7: Verify**

```bash
git status --short
git log --oneline -2
```

Expected: `git status` shows only `claude-progress.md` untouched (Task 3); HEAD is the CI commit, preceded by the Task 1 commit.

---

### Task 3: Record session progress and final verification

**Files:**
- Modify: `claude-progress.md` — append a Session 011 entry
- Test: smoke test + both gates (final)

**Interfaces:**
- Consumes: both commits from Tasks 1 and 2
- Produces: commit `docs: record format-lint gate toolchain landing (Session 011)`; a fully clean working tree

- [ ] **Step 1: Append the progress entry**

Append to the end of `claude-progress.md`:

```markdown
## Session 011 (2026-08-08) — format-lint gate toolchain

- Goal: land the deterministic quality gate for Rust changes, shared by local
  Windows/Linux/macOS runs and CI, without duplicated logic.
- What landed:
  - `scripts/format-lint-steps.json` (v2) — single source of truth: the 5 gate
    steps (fmt, diff, forbidden paths, clippy, test) and the 6
    forbidden-diff-path patterns.
  - `scripts/format-lint.sh` and
    `.agents/skills/format-lint/scripts/format-lint.ps1` — both gates read and
    execute the manifest; flags only transform its default steps.
  - `scripts/test-format-lint.sh` — 24-check smoke test asserting both gates'
    exit codes, forbidden-path handling, flag transforms, manifest parsing,
    per-pattern matching, and mirror byte-identity.
  - `.claude/skills/format-lint/` mirror (byte-identical) + SKILL.md.
  - `.gitignore` cleanup; `Cargo.lock` committed for reproducible app builds.
  - CI: smoke test wired into the ubuntu `checks` and `windows` jobs.
- Verification: smoke test 24/24; both full gates pass with tests; mirror
  byte-identity verified; `.claude/settings.local.json` untouched.
- Out of scope: pre-commit hook changes.
```

- [ ] **Step 2: Commit the progress entry**

```bash
git add claude-progress.md
git commit -m "docs: record format-lint gate toolchain landing (Session 011)"
```

- [ ] **Step 3: Final verification**

```bash
git status --short
git log --oneline -4
bash scripts/test-format-lint.sh
```

Expected: `git status` fully clean; the log shows the three commits from Tasks 1–3 on top of the previous HEAD; smoke test 24/24.

- [ ] **Step 4: Report**

Summarize: the three commit SHAs, the verification evidence (smoke 24/24, both gates green, mirror sync), and the known accepted cost (ubuntu `checks` job wastes a few seconds on the failing `--all-features` clippy build). Do not push.
