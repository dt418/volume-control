# Rust Inner Check Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make AI coding agents catch Rust type/API errors after each small logical Rust change by adding a mandatory lightweight `cargo check` development loop.

**Architecture:** Put the canonical inner-loop contract in shared agent guidance and keep existing commit/CI/release gates unchanged. Prefer direct documentation over new infrastructure; add a helper script only if implementation inspection shows an established lightweight script pattern where it prevents command drift.

**Tech Stack:** Rust 2021, Cargo workspace, Tauri 2, `windows-sys` 0.52, repository Markdown agent instructions, POSIX/PowerShell tooling where already established.

## Global Constraints

- The inner command is exactly `cargo check --workspace --no-default-features`.
- Run it after each small logical Rust change, not after documentation-only or frontend-only changes.
- A failed inner check blocks further feature implementation until the root cause is resolved.
- For `E0308`, `E0277`, `E0599`, and `E0061`, verify the actual type/signature contract before choosing a fix.
- External API fixes must use the dependency version resolved by this project; do not rely on remembered generic examples.
- Give extra scrutiny to Tauri, `windows-sys`, `winit`, `tray-icon`, WASAPI, and COM boundaries.
- Do not broadly update `Cargo.lock` as a side effect of compiler-error repair.
- Existing format, Clippy, tests, record-keeping, CI, and shipping gates remain authoritative and are not weakened.
- Do not refactor application code as part of this change.

---

### Task 1: Add the canonical Rust inner-loop contract

**Files:**
- Modify: `AGENTS.md`
- Modify only if it contains independent agent workflow rules rather than a pointer to `AGENTS.md`: `CLAUDE.md`
- Modify the repository's Codex-specific instruction file only if it duplicates coding workflow policy; otherwise leave it referencing the shared contract.
- Modify: `feature_list.json`
- Modify: `claude-progress.md`

**Interfaces:**
- Consumes: existing format/lint/test and shipping rules in `AGENTS.md` and `GUARDRAILS.md`.
- Produces: one canonical documented command, `cargo check --workspace --no-default-features`, and a stop-on-failure policy visible to coding agents.

- [ ] **Step 1: Inspect agent instruction ownership before editing**

Read `AGENTS.md`, `CLAUDE.md`, and the relevant `.codex` instruction/config files. Identify which file owns shared coding workflow policy. Do not duplicate a full policy into tool-specific files when they already defer to `AGENTS.md`.

Expected: `AGENTS.md` remains the canonical shared location unless repository text explicitly establishes another owner.

- [ ] **Step 2: Add the inner development loop to the canonical guidance**

Add a focused section with this behavioral contract:

```markdown
## Rust inner development loop

After each small logical change to Rust code, run:

`cargo check --workspace --no-default-features`

If it fails, stop additional feature implementation and resolve the compiler error before continuing. Read the complete diagnostic and make one minimal, evidence-based correction at a time.

For type/API contract errors such as `E0308`, `E0277`, `E0599`, and `E0061`, verify the actual signature/type used by the project's resolved dependency version before changing code. Apply this especially at Tauri, `windows-sys`, `winit`, `tray-icon`, WASAPI, and COM boundaries. Do not broadly update `Cargo.lock` merely to make a compiler error disappear.

This inner check applies to Rust changes only. It does not replace the full format, Clippy, test, record-keeping, CI, or shipping gates below.
```

Preserve the existing full quality gate verbatim unless a small connective sentence is required.

- [ ] **Step 3: Align tool-specific agent guidance without policy duplication**

If `CLAUDE.md` or Codex-specific instructions independently prescribe a Rust coding loop, replace the conflicting/duplicated portion with a short reference to the canonical shared rule. If they already inherit or reference `AGENTS.md`, make no change.

Expected: all agents receive the same command and stop-on-failure semantics, with one source of truth.

- [ ] **Step 4: Update required repository records**

Update `feature_list.json` and `claude-progress.md` according to the existing record schema, describing the Rust inner-check guardrail as developer tooling/process behavior rather than an application feature.

- [ ] **Step 5: Verify documentation and record guards**

Run:

```bash
git diff --check
sh scripts/check-records.sh --branch
```

Expected: both commands exit 0 and the diff shows no weakening of existing gates.

- [ ] **Step 6: Commit the independently reviewable documentation change**

```bash
git add AGENTS.md CLAUDE.md .codex feature_list.json claude-progress.md
git status --short
git commit -m "chore: require Rust inner compile checks"
```

Before `git add`, omit any listed path that was correctly left unchanged. Expected: commit contains only agent-policy and required record updates.

---

### Task 2: Decide whether a helper command is justified

**Files:**
- Inspect: `scripts/`
- Create only if justified by existing conventions: `scripts/rust-check.sh`
- Create only if the repository normally maintains Windows counterparts for developer commands: `scripts/rust-check.ps1`
- Test only if a helper is added: existing script self-test location/pattern under `scripts/`
- Modify if a helper is added: `AGENTS.md`, `feature_list.json`, `claude-progress.md`

**Interfaces:**
- Consumes: canonical command `cargo check --workspace --no-default-features` from Task 1.
- Produces, only if justified: a transparent wrapper that returns Cargo's exit status and performs no extra full-gate work.

- [ ] **Step 1: Inspect existing lightweight script conventions**

Review `scripts/` and its script self-tests. A helper is justified only when the repository already uses named wrappers for frequently repeated developer checks and the wrapper reduces cross-agent command drift.

Decision rule: if direct Cargo commands are the established pattern for single checks, skip the helper and proceed to Step 5. Do not create infrastructure merely because the spec permits it.

- [ ] **Step 2: If justified, write a failing wrapper self-test first**

Follow the repository's existing shell-script self-test structure. The test must prove that the helper invokes the workspace check with `--no-default-features` and propagates a non-zero Cargo exit status. Use a fake `cargo` earlier on `PATH` rather than compiling the workspace inside the wiring test.

Expected before implementation: the test fails because the helper does not exist.

- [ ] **Step 3: If justified, implement the minimal helper**

POSIX implementation must be equivalent to:

```sh
#!/bin/sh
set -eu
exec cargo check --workspace --no-default-features
```

If a PowerShell counterpart is required by repository convention, it must run the same Cargo arguments and exit with Cargo's exit code. Do not add formatting, Clippy, tests, dependency updates, or retries.

- [ ] **Step 4: If justified, run the wrapper self-test and records guard**

Run the exact existing self-test entry point discovered in Step 1, then:

```bash
git diff --check
sh scripts/check-records.sh --branch
```

Expected: wrapper test passes; both repository guards exit 0.

- [ ] **Step 5: Record the helper decision**

If no helper is added, leave a concise note in `claude-progress.md` that direct Cargo invocation was retained to avoid unnecessary infrastructure. If a helper is added, update `AGENTS.md`, `feature_list.json`, and `claude-progress.md` using the repository's existing record format and make the helper the documented convenience entry point while retaining the exact Cargo command semantics.

- [ ] **Step 6: Commit only if Task 2 produced changes beyond Task 1**

For a helper:

```bash
git add scripts AGENTS.md feature_list.json claude-progress.md
git status --short
git commit -m "chore: add Rust inner check helper"
```

For a no-helper decision that changes only the required progress record:

```bash
git add claude-progress.md
git status --short
git commit -m "docs: record Rust check helper decision"
```

Expected: no empty commit.

---

### Task 3: Verify the complete guardrail change

**Files:**
- Verify: all files changed in Tasks 1-2
- Do not modify application code unless verification exposes a pre-existing issue that is explicitly taken out of scope and reported instead.

**Interfaces:**
- Consumes: agent-policy changes and optional helper from Tasks 1-2.
- Produces: evidence that the new inner loop is documented and the existing authoritative quality gates still pass.

- [ ] **Step 1: Run the new inner check**

```bash
cargo check --workspace --no-default-features
```

Expected: exit 0. If it fails, investigate the complete diagnostic; do not change unrelated application code merely to force this guardrail task green.

- [ ] **Step 2: Run formatting and whitespace gates**

```bash
cargo fmt --all --check
git diff --check
```

Expected: both exit 0.

- [ ] **Step 3: Run strict Clippy**

```bash
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
```

Expected: exit 0 with no project warnings suppressed.

- [ ] **Step 4: Run workspace tests**

```bash
cargo test --workspace --no-default-features
```

Expected: exit 0.

- [ ] **Step 5: Run record and existing gate self-tests**

```bash
sh scripts/check-records.sh --branch
bash scripts/test-format-lint.sh
bash scripts/test-check-records.sh
bash scripts/test-ship.sh
```

Expected: all exit 0.

- [ ] **Step 6: Review the final diff for scope**

```bash
git diff main...HEAD -- AGENTS.md CLAUDE.md .codex scripts feature_list.json claude-progress.md docs/superpowers
```

Expected: only agent instructions, optional lightweight helper/test, required records, and approved spec/plan documentation changed. Existing commit/CI/release requirements are not weakened.

- [ ] **Step 7: Commit any final record-only verification update if required by repository policy**

If verification requires a substantive record update under the repository's rules, stage only that record change and commit it. Otherwise make no commit.

Expected: branch is clean after verification and ready for review/PR.