# Cross-Platform Testing and Release Safety Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Close the verified orchestration, E2E evidence, platform-coverage, and release-gating gaps while keeping Windows-first feedback fast and preventing an Ubuntu/macOS release artifact from bypassing validation.

**Architecture:** Preserve the existing Rust/Vitest/Tauri/WDIO/Pilot pyramid. Add a reproducible Codex configuration contract, strengthen the already-landed WDIO wrapper with fail-closed evidence, keep Pilot local-only, and move release packaging behind one SHA-bound reusable desktop validation workflow. Windows WDIO remains the PR gate; Linux/macOS full validation runs on master merges and releases.

**Tech Stack:** Rust 1.82 workspace, Tauri 2, WebdriverIO 9, @wdio/tauri-service, Tauri Pilot CLI for local diagnostics, GitHub Actions, PowerShell, Bash, Python tomllib, Vitest, GTK4/libadwaita, WebKitGTK 4.1.

## Global Constraints

- Keep the existing Tauri, WDIO, Pilot, GitHub Actions, and platform abstraction boundaries.
- Do not add screenshot-diff tests, paid Mac infrastructure, self-hosted runners, Docker GUI infrastructure, or universal macOS builds.
- The release WDIO target is the debug/plugin binary; production binaries are validated separately for startup, packaging, and debug-plugin exclusion.
- Tauri Pilot is local/debug-only and never changes the production release exit code.
- Windows WDIO runs on every pull request; full Windows/Linux/macOS validation runs on master merges and release validation.
- Hosted Linux/Xvfb does not claim real Wayland, tray, hardware audio, global-hotkey, or multi-monitor coverage.
- Hosted macOS does not claim TCC/Accessibility, menu-bar, hardware CoreAudio, Retina/multi-monitor, or Gatekeeper confidence.
- Every substantive code, script, workflow, or skill change updates feature_list.json and claude-progress.md together.
- The repository gate remains cargo fmt --all --check, git diff --check, clippy with -D warnings, workspace tests, and the records guard.

---

### Task 1: Make the coordinator configuration reproducible and depth-safe

Files:
- Modify .codex/config.toml:47-77
- Modify .codex/ORCHESTRATOR.md:30-43
- Modify .codex/AGENTS.md:12-22
- Create scripts/test-codex-config.py
- Modify .github/workflows/ci.yml
- Modify feature_list.json and claude-progress.md

Interfaces:
- python scripts/test-codex-config.py [repo-root] exits 0 only when configured role files, depth/thread limits, and orchestration documentation are consistent.
- The contract requires max_threads == 4, max_depth == 2, and the four profile paths referenced by agents.* to exist inside .codex/agents/.
- A clean-checkout check extracts git archive HEAD into a temporary directory and invokes the same script there.

- [ ] Step 1: Write the failing config contract

Create scripts/test-codex-config.py with Python 3.11+ tomllib. Resolve the repository root from the optional argument or script parent, parse .codex/config.toml, assert max_threads == 4, assert max_depth == 2, assert the four configured profile paths are relative and exist, assert .codex/ORCHESTRATOR.md contains the coordinator handoff, and exit nonzero with the missing path or mismatched value.

- [ ] Step 2: Run the contract RED

Run: python scripts/test-codex-config.py

Expected: FAIL because the committed config still has max_depth = 1.

- [ ] Step 3: Raise the depth cap and synchronize documentation

Change .codex/config.toml to max_depth = 2. Change the cost/quality paragraph in .codex/ORCHESTRATOR.md from max_depth = 1 to max_depth = 2. Update .codex/AGENTS.md to list coordinator, planner, implementer, and release reviewer alongside the existing read-only roles. Do not activate commented model IDs until a restarted runtime confirms the catalog.

- [ ] Step 4: Run the contract GREEN and verify a clean archive

Run python scripts/test-codex-config.py. Then create a temporary directory with PowerShell, extract git archive HEAD into that directory, run python scripts/test-codex-config.py against the extracted absolute path, and remove the directory in finally. Expected: both invocations pass and the extracted tree contains every referenced profile.

- [ ] Step 5: Wire the contract into the shared checks job

Add a Python setup step and run python scripts/test-codex-config.py in the Ubuntu checks job before the Rust gate. Keep Windows and macOS jobs independent of the config contract.

- [ ] Step 6: Record and commit

Update the feature record with the contract command and clean-archive evidence, then run:
git add .codex scripts/test-codex-config.py .github/workflows/ci.yml feature_list.json claude-progress.md
git commit -m "ci: make coordinator configuration reproducible"

Rollback point: revert this commit only if the Codex runtime rejects nested depth 2; do not revert tracked role profiles while leaving config references active.

---

### Task 2: Make the existing WDIO E2E gate fail closed on evidence and errors

Files:
- Modify e2e/tauri/package.json and package-lock.json
- Modify e2e/tauri/wdio.conf.ts
- Modify e2e/tauri/support/artifacts.ts and commands.ts
- Modify e2e/tauri/support/artifacts.test.ts and commands.test.ts
- Modify e2e/tauri/specs/runtime.e2e.ts
- Modify scripts/verify-tauri-e2e.ps1 and scripts/verify-tauri-e2e.sh
- Modify .github/workflows/ci.yml
- Modify feature_list.json and claude-progress.md

Interfaces:
- writeE2eManifest(outputRoot: string, manifest: E2eManifest): Promise<string> writes manifest.json.
- assertE2eEvidence(outputRoot: string, expectedSpecs: string[]): Promise<void> fails when JUnit, manifest, or required result entries are absent.
- collectRuntimeErrors(browser: E2eBrowser): Promise<{ frontend: string[]; backend: string[] }> returns captured errors.
- timingReport() writes p50/p95 JSON and reads TAURI_E2E_P95_BOOTSTRAP_MS and TAURI_E2E_P95_IPC_MS.

- [ ] Step 1: Add failing evidence contract tests

Add fixtures for a missing JUnit file, a manifest missing one expected spec, and a non-empty frontend error list. Assert each helper rejects with the exact missing artifact or error category.

- [ ] Step 2: Run the evidence tests RED

Run: npm test --prefix e2e/tauri -- --test-name-pattern "manifest|JUnit|runtime errors"

Expected: FAIL because no JUnit reporter or manifest assertion exists.

- [ ] Step 3: Add and pin the JUnit reporter

Add @wdio/junit-reporter at version 9.30.1, run npm install --package-lock-only --prefix e2e/tauri, and configure wdio.conf.ts to write JUnit XML under TAURI_E2E_OUTPUT/junit. Keep the spec reporter.

- [ ] Step 4: Implement manifest and zero-error assertions

Implement the artifact helpers with deterministic spec names and sanitized paths. Add the runtime assertion that captured errors are empty. Allow only the documented unavailable-audio/degraded-hotkey messages; all other errors fail.

- [ ] Step 5: Make both wrappers verify evidence after WDIO exits

Preserve the WDIO exit code, then call the evidence helper when the run exits 0. Require JUnit, manifest.json, timings.json, and one result entry for every requested surface. Always clean temporary capability/bridge files.

- [ ] Step 6: Make CI artifact uploads fail closed

Change the Windows, Linux, and macOS E2E upload steps from if-no-files-found: warn to if-no-files-found: error. Upload JUnit, manifest, timing, screenshots, snapshots, and frontend/backend logs under if: always().

- [ ] Step 7: Run the focused suite GREEN

Run:
npm test --prefix e2e/tauri
npm run typecheck --prefix e2e/tauri
npm run test:contract --prefix e2e/tauri
npm run test:production-exclusion --prefix e2e/tauri

Expected: all contracts pass; negative fixtures fail only inside their assertions.

- [ ] Step 8: Commit

git add e2e/tauri scripts/verify-tauri-e2e.* .github/workflows/ci.yml feature_list.json claude-progress.md
git commit -m "test: make Tauri E2E evidence fail closed"

Rollback point: revert only reporter/manifest wrapper changes if a provider cannot emit JUnit, while retaining runtime error and cleanup checks.

---

### Task 3: Add explicit provider, timing, and Windows hotkey evidence

Files:
- Modify e2e/tauri/wdio.conf.ts and support/artifacts.ts
- Modify e2e/tauri/support/artifacts.test.ts
- Create e2e/tauri/test-provider.mjs
- Create scripts/verify-hotkey-latency.ps1
- Modify e2e/pilot/README.md, README.md, README.vi.md
- Modify feature_list.json and claude-progress.md

Interfaces:
- node e2e/tauri/test-provider.mjs --provider embedded|tauri-driver --platform windows|linux|macos exits nonzero when the requested provider is unavailable.
- pwsh -NoProfile -File scripts/verify-hotkey-latency.ps1 -Release -Iterations 10 -OutputRoot output/manual/hotkey-latency writes hotkey-latency.json and hotkey-latency.txt for target/release/VolumeControl.exe.
- The latency report records keydown timestamp, surface-visible timestamp, delta, p50, p95, OS build, app SHA, and configured shortcut.

- [ ] Step 1: Add provider-selection contract tests

Pass for an installed embedded provider, reject tauri-driver when its executable is absent, and reject an unknown provider. The test must not rewrite E2E_DRIVER_PROVIDER.

- [ ] Step 2: Make provider selection explicit

Require E2E_DRIVER_PROVIDER in CI instead of defaulting silently. Set embedded explicitly in each desktop job. Permit tauri-driver only after its preflight contract passes; never silently fall back.

- [ ] Step 3: Add deterministic timing budgets

Calculate p50/p95 from monotonic samples. Enforce only bootstrap-to-ready and deterministic IPC budgets with initial limits of 3000 ms and 250 ms. Record render/screenshot/audio timings without blocking.

- [ ] Step 4: Add the Windows manual hotkey latency probe

Reuse scripts/win32_pinvoke.cs and verify-vol011.ps1 window-discovery conventions. Send the configured shortcut with keybd_event, poll for the Volume Mixer window using Stopwatch, repeat ten times with key release cleanup in finally, and terminate only the child process started by the script. Do not run this probe on Linux/macOS or use it as synthetic Pilot evidence.

- [ ] Step 5: Document evidence boundaries

Document that the Windows probe is real OS integration evidence; WDIO shortcut cards prove only UI/configuration; Linux Xvfb and hosted macOS do not prove native shortcut delivery. Add exact commands and report locations to both READMEs and the Pilot README.

- [ ] Step 6: Run focused checks and commit

Run node e2e/tauri/test-provider.mjs --provider embedded --platform windows, npm test --prefix e2e/tauri -- --test-name-pattern "timing|provider", and pwsh -NoProfile -File scripts/verify-hotkey-latency.ps1 -? (usage only).
Expected: contracts pass and the usage command does not start an app.

Commit with:
git add e2e/tauri scripts/verify-hotkey-latency.ps1 README.md README.vi.md feature_list.json claude-progress.md
git commit -m "test: add explicit provider and Windows hotkey evidence"

---

### Task 4: Apply the balanced CI schedule without hiding shared-code regressions

Files:
- Modify .github/workflows/ci.yml and release.yml
- Modify scripts/test-ship.sh
- Modify feature_list.json and claude-progress.md

Interfaces:
- Windows CI remains unconditional for pull_request and push to master.
- Linux and macOS CI run on push to master and are skipped only for ordinary pull requests.
- Every desktop E2E upload uses if-no-files-found: error.

- [ ] Step 1: Add a workflow scheduling contract

Extend scripts/test-ship.sh with assertions that the Windows job has no pull-request-only skip, Linux/macOS jobs use if: github.event_name != 'pull_request', and all three E2E upload steps use if-no-files-found: error. The new assertions must fail against the current unconditional Linux/macOS jobs.

- [ ] Step 2: Run the scheduling contract RED

Run bash scripts/test-ship.sh. Expected: new assertions fail while existing ship assertions pass.

- [ ] Step 3: Apply event conditions

Add Linux and macOS job conditions without changing dependency installation or E2E commands. Keep the Windows and checks jobs on pull requests. Preserve the committed Ubuntu release dependency fix and pkg-config probes.

- [ ] Step 4: Run the scheduling contract GREEN

Run bash scripts/test-ship.sh and parse both workflow files with PyYAML. Expected: all static checks pass and no job loses release validation.

- [ ] Step 5: Commit

git add .github/workflows/ci.yml .github/workflows/release.yml scripts/test-ship.sh feature_list.json claude-progress.md
git commit -m "ci: balance desktop validation by event"

Rollback point: remove only the Linux/macOS pull-request conditions if hosted merge validation becomes unavailable; retain fail-closed artifacts and the Windows PR gate.

---

### Task 5: Put release packaging behind SHA-bound desktop validation

Files:
- Create .github/workflows/desktop-validation.yml
- Modify .github/workflows/release.yml and scripts/package.sh
- Create scripts/verify-release-metadata.sh and scripts/test-release-workflow.sh
- Create scripts/test-fixtures/release-valid/build-metadata.json and scripts/test-fixtures/release-valid/volumecontrol-test.tar.gz
- Create scripts/test-fixtures/release-mismatch/build-metadata.json and scripts/test-fixtures/release-wrong-platform/build-metadata.json
- Modify feature_list.json and claude-progress.md

Interfaces:
- Reusable workflow inputs: release_mode boolean and release_tag string.
- Artifacts: validated-windows-GITHUB_SHA, validated-macos-GITHUB_SHA, and validated-ubuntu-GITHUB_SHA. Each contains the package, build-metadata.json, JUnit/E2E manifest, and platform logs.
- scripts/verify-release-metadata.sh accepts an artifact directory, expected commit SHA, and platform argument; it exits nonzero when metadata, platform, checksum input, or commit SHA mismatches.

- [ ] Step 1: Write the release metadata contract

Create shell fixtures for valid metadata, mismatched SHA, wrong platform, and missing package. Assert only the valid fixture exits 0. Metadata contains commit_sha, platform, runner_arch, rustc, node, artifact, and artifact_sha256.

- [ ] Step 2: Run the metadata contract RED

Run bash scripts/verify-release-metadata.sh scripts/test-fixtures/release-valid "$(git rev-parse HEAD)" ubuntu before creating the fixture files. Expected: a clear missing-metadata failure.

- [ ] Step 3: Extract the release matrix into the reusable workflow

Move the Windows/macOS/Ubuntu release build steps into desktop-validation.yml. Keep the Ubuntu install block from release.yml, including libwebkit2gtk-4.1-dev, libgtk-3-dev, and both pkg-config probes. Each matrix job runs platform E2E/core checks before tauri build, calls scripts/package.sh, writes metadata with github.sha and uname -m/file, and uploads the SHA-named artifact.

- [ ] Step 4: Make release consume only validated artifacts

Change release.yml so validate calls the reusable workflow with release_mode true. Make the GitHub Release job need validate, download only SHA-named artifacts, run verify-release-metadata.sh for every platform, generate SHA256SUMS.txt, and publish. Remove the old direct platform build matrix from the caller; it must not rebuild an unvalidated binary.

- [ ] Step 5: Add release workflow contract tests

Assert that release.yml calls the reusable workflow, the publish job needs validation, no direct tauri build remains in the caller, and the metadata verifier runs before gh release create/upload.

- [ ] Step 6: Run release contracts and local package rehearsal

Run bash scripts/test-release-workflow.sh and bash scripts/verify-release-metadata.sh scripts/test-fixtures/release-valid "$(git rev-parse HEAD)" ubuntu. Valid metadata passes and the mismatch/wrong-platform fixture directories fail. A local Windows package rehearsal may use scripts/package.sh only when target/release/VolumeControl.exe exists; do not fabricate Linux/macOS artifacts.

- [ ] Step 7: Commit

git add .github/workflows/desktop-validation.yml .github/workflows/release.yml scripts/package.sh scripts/verify-release-metadata.sh scripts/test-release-workflow.sh feature_list.json claude-progress.md
git commit -m "release: publish only validated desktop artifacts"

Rollback point: restore the previous release caller only if the reusable workflow cannot expose artifacts in the same run; retain a hard validation prerequisite and never reintroduce an unvalidated direct tag build.

---

### Task 6: Publish the manual cross-platform checklist and signing boundary

Files:
- Create docs/testing/cross-platform-release-checklist.md
- Modify README.md, README.vi.md, CHANGELOG.md
- Modify feature_list.json and claude-progress.md

Interfaces:
- The checklist covers Windows manual, Ubuntu/Xvfb, WSLg, hosted macOS, release inspection, and unsupported claims.
- Every manual run records app SHA, OS version, architecture, display topology, audio endpoint, configured shortcut, measured latency, and evidence directory.
- macOS signing is split into current ad-hoc validation and a future Developer ID/notarization workflow requiring GitHub secrets; no secret appears in repository files.

- [ ] Step 1: Write the checklist

Include exact commands for frontend tests/build, WDIO wrappers, xvfb-run -a cargo test --features gtk-renderer, the Windows hotkey probe, uname -m, file, codesign --verify, and plutil -lint. Mark tray, TCC, hardware audio, Wayland compositor, and multi-monitor claims as partial/manual.

- [ ] Step 2: Update README release claims

Replace broad statements that imply hosted CI proves native hotkeys/audio/tray with links to the checklist and the Strong/Partial/Compile-only classifications. Document that the current macOS package is ad-hoc signed and public distribution needs Developer ID/notarization credentials.

- [ ] Step 3: Update changelog and records

Add release-gate, evidence, and platform-boundary changes to the unreleased section. Record hosted workflow URLs and artifact paths only after hosted runs complete; do not mark passing on local Windows evidence alone.

- [ ] Step 4: Commit

git add docs/testing README.md README.vi.md CHANGELOG.md feature_list.json claude-progress.md
git commit -m "docs: define cross-platform release evidence boundaries"

---

### Task 7: Final verification, adversarial review, and handoff

Files:
- Verify all files changed by Tasks 1-6
- Update feature_list.json, claude-progress.md, and session-handoff.md

- [ ] Step 1: Run the local quality gate

Run cargo fmt --all --check, git diff --check, cargo clippy --workspace --all-targets --no-default-features -- -D warnings, cargo test --workspace --no-default-features, npm test --prefix frontend, npm run build --prefix frontend, npm run test:e2e --prefix e2e/tauri, npm run test:pilot-contract --prefix e2e/tauri, python scripts/test-codex-config.py, bash scripts/test-ship.sh, bash scripts/test-release-workflow.sh, and bash scripts/check-records.sh --staged.
Expected: all commands exit 0; Pilot runtime remains explicitly local-only if the CLI/toolchain is unavailable.

- [ ] Step 2: Run hosted evidence in Windows-first order

Run the Windows PR gate and inspect JUnit, manifest, logs, screenshots, timing report, and cleanup state. On the master merge run, inspect Linux/Xvfb and macOS artifacts, including gdk-3.0/webkit2gtk-4.1 probes and architecture metadata.

- [ ] Step 3: Execute the release rehearsal

Run the tag/workflow-dispatch release path against a disposable prerelease tag. Verify every published artifact has matching metadata/checksum and no artifact is published when one matrix platform fails.

- [ ] Step 4: Perform release review

Review the complete diff for crash resistance, security, shortcut latency, accessibility, UI/UX regressions, configuration behavior, cleanup, and platform claim accuracy. Re-run all failed checks after every genuine finding; do not waive missing evidence.

- [ ] Step 5: Record final handoff

Update feature_list.json, claude-progress.md, and session-handoff.md with exact commands, exit codes, hosted run IDs, artifact paths, platform limitations, and final release decision. Mark passing only after the hosted Ubuntu release build completes beyond the gdk-sys step.

## Self-review against the approved spec

- Orchestration reproducibility and max_depth = 2: Task 1.
- WDIO JUnit, manifest, zero-error, timing, and cleanup contract: Task 2.
- Explicit provider selection and real Windows shortcut evidence: Task 3.
- Balanced PR/master scheduling and artifact retention: Task 4.
- SHA-bound release validation, package metadata, and no tag bypass: Task 5.
- Windows/Linux/WSLg/macOS evidence boundaries and signing limitations: Task 6.
- Full gate, hosted evidence, release rehearsal, and adversarial review: Task 7.
- Already-landed Ubuntu dependency fix from commit a5e90a0 is baseline and is not recreated by these tasks.

No implementation should begin until this plan is explicitly selected for execution.
