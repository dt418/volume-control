---
name: winget-publish
description: Use when publishing or updating VolumeControl in winget, generating or validating dt418.VolumeControl manifests, opening or updating the microsoft/winget-pkgs PR, checking Validation-Completed, or diagnosing winget validate/install and Windows Sandbox errors.
---

# Publish VolumeControl to winget

Package: `dt418.VolumeControl`. Upstream: `microsoft/winget-pkgs`.
The repo's `winget/` tree is the reviewable snapshot; the upstream fork is a
separate checkout. Local validation is fast but only proves the manifest is
well formed. The upstream GitHub App runs the real ten-step validation
(download, hash, AV, clean install, ARP/icon checks) in roughly 35-40 minutes.

## Invariants

- One package version per upstream PR. Only manifest files.
- Never remove a version directory that is already merged upstream.
- Never push a commit just to "wake" validation. A push resets the labels and
  the 35-40 minute pipeline.
- Do not open a second PR for the same package while one is open.
- `Validation-Completed` is not publication. Moderator approval, auto-merge,
  and the publish pipeline must finish first.
- There is no paid fast lane and no MCP API into the validation backend.

## Run from any directory

Install the two user-level commands once (from the repo root):

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install-winget-commands.ps1
```

They are written to `%LOCALAPPDATA%\Microsoft\WindowsApps` (already on PATH)
and call this repo by absolute path, so they work from any current directory:

```powershell
winget-manifest path
winget-manifest validate
winget-manifest show
winget-manifest install
winget-manifest validate -Version 0.1.10
winget-status
```

Re-run the installer if the repo moves. Remove the commands with
`-Uninstall`.

Without the shims, run the scripts from the repo root. PowerShell needs
`.\`; Git Bash needs `./`:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\winget-manifest.ps1 validate
powershell -ExecutionPolicy Bypass -File .\scripts\show-winget-validation.ps1
```

`scripts/winget-manifest.ps1` and `scripts/install-local-winget.ps1` resolve
the newest semver directory and validate before installing.
`scripts/SandboxTest.ps1` does the same for an isolated Windows Sandbox run.

## 1. Prerequisites

- GitHub release tag `vX.Y.Z` and asset
  `VolumeControl_X.Y.Z_x64-setup.exe` are published.
- The installer filename is exactly
  `VolumeControl_<version>_x64-setup.exe`; the generator rejects anything else.
- `gh auth status` works. Fork `dt418/winget-pkgs` exists.
- Local manifest installs need the one-time elevated setting:
  `winget settings --enable LocalManifestFiles`.
- Windows Sandbox is optional. Enable it from elevated Windows PowerShell 5.1,
  not PowerShell 7 (`Class not registered` is the PS7 DISM bug):
  ```powershell
  Enable-WindowsOptionalFeature -Online -FeatureName Containers-DisposableClientVM -All -NoRestart
  ```

## 2. Generate and verify manifests

Download the exact release asset and checksum manifest, then compare hashes:

```powershell
$v = "0.1.10"
$asset = "VolumeControl_${v}_x64-setup.exe"
Invoke-WebRequest "https://github.com/dt418/volume-control/releases/download/v$v/$asset" -OutFile $asset
Invoke-WebRequest "https://github.com/dt418/volume-control/releases/download/v$v/SHA256SUMS.txt" -OutFile SHA256SUMS.txt
Get-FileHash $asset -Algorithm SHA256
Select-String $asset SHA256SUMS.txt
```

Generate into the repo snapshot. On Windows use Git Bash for `bash`
(`bash scripts/...`, not the WSL shim):

```bash
bash scripts/generate-winget-manifest.sh v0.1.10 /path/to/VolumeControl_0.1.10_x64-setup.exe winget
```

This writes the three 1.12.0 files under
`winget/dt418.VolumeControl/0.1.10/`. `scripts/deploy-winget.sh` is the
orchestrator; it writes to `output/winget` by default and also prepares a PR
body, so copy its manifests into `winget/` if you use it.

Validate and install-test on the host:

```powershell
winget-manifest validate -Version 0.1.10
winget-manifest install -Version 0.1.10
winget list --name VolumeControl
```

Record the actual install output, ARP `DisplayVersion`, and installed exe file
version. `winget list --id` can miss local-manifest installs; use `--name` or
the ARP entry. Do not claim local install evidence you did not observe.

Optional isolated check:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/SandboxTest.ps1 -Version 0.1.10
```

## 3. Submit or update the upstream PR

Check for an existing open PR first:

```powershell
gh pr list -R microsoft/winget-pkgs --author dt418 --state open `
  --json number,title,headRefName,url
```

If a new-package PR is still open and unmerged (for example #454508), update
that PR to the new version. Do not open a duplicate. If the package is already
merged, create a new branch and a new-version PR instead.

Sparse-checkout the fork (adapt the existing head branch name from the PR):

```bash
# If output/winget-fork already exists, cd into it and fetch instead of cloning.
git clone --depth 1 --filter=blob:none --sparse https://github.com/dt418/winget-pkgs.git output/winget-fork
cd output/winget-fork
git sparse-checkout set manifests/d/dt418/VolumeControl
git fetch origin dt418.VolumeControl-0.1.5
git switch dt418.VolumeControl-0.1.5
```

Copy the manifest set to the upstream path, removing the superseded version
only when the PR is still an unmerged new-package submission:

```bash
mkdir -p manifests/d/dt418/VolumeControl/0.1.10
cp ../../winget/dt418.VolumeControl/0.1.10/*.yaml manifests/d/dt418/VolumeControl/0.1.10/
git rm -r manifests/d/dt418/VolumeControl/0.1.9   # only if this PR is still open
git add manifests/d/dt418/VolumeControl
git commit -m "Update dt418.VolumeControl to 0.1.10"
git push origin HEAD:dt418.VolumeControl-0.1.5
```

Update the PR title and body from the repository root. Regenerate
`output/winget/pr-body.md` for the target version with the orchestration script
or write it from the template below; never reuse a previous version's body:

```powershell
cd ../..   # back to the repository root after the fork commands
gh pr edit 454508 -R microsoft/winget-pkgs `
  --title "New package: dt418.VolumeControl version 0.1.10" `
  --body-file output/winget/pr-body.md
```

If this is a new-version submission with no existing PR:

```powershell
gh pr create -R microsoft/winget-pkgs --base master `
  --head "dt418:dt418.VolumeControl-0.1.10" `
  --title "New version: dt418.VolumeControl version 0.1.10" `
  --body-file output/winget/pr-body.md
```

Use `New package:` for the first submission and `New version:` after the
package is in the upstream catalog. The PR body must include:

- no duplicate open PR and exactly one package version;
- only manifest files changed;
- `winget validate` result;
- local `winget install --manifest` evidence (OS, winget version, output,
  installed version);
- installer URL and manifest schema version.

A compact body that matches the upstream checklist:

```markdown
- [x] Checked that there aren't other open pull requests for the same manifest update/change
- [x] This PR only modifies one (1) manifest
- [x] Validated manifest locally with `winget validate --manifest <path>` (result: `Manifest validation succeeded.`)
- [x] Tested manifest locally with `winget install --manifest <path>`:
  <install output and installed version>
- [x] Manifest conforms to the 1.12 schema

Installer: https://github.com/dt418/volume-control/releases/download/v0.1.10/VolumeControl_0.1.10_x64-setup.exe
```

Do not include unrelated docs, spelling, or tooling changes in the manifest
PR.

## 4. Monitor upstream validation

Run the checker first to refresh the snapshot, then display the cached status:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/check-winget-validation.ps1
winget-status
```

`winget-status` only reads `%LOCALAPPDATA%\VolumeControl\winget-validation\` and
never queries GitHub; without a prior checker/task run it throws, and its
output can be stale.

The checker writes status and an append-only log under
`%LOCALAPPDATA%\VolumeControl\winget-validation\`. Install a 5-minute watcher
with:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/install-winget-validation-task.ps1
```

The bundled watchers are pinned to the current new-package submission,
**PR #454508 / version 0.1.9**. Before using them for another PR, update the
default `PrNumber` and expected version in both checkers (the scheduled task
inherits them), change `PR_NUMBER` in
`.github/workflows/winget-publish-watch.yml`, and re-enable the scheduled task
or workflow if it self-disabled. Otherwise they keep reporting the old
submission.

The scheduled task runs with `-PingOnGreen -StopOnTerminal`: it posts one short
moderator ping when validation turns green and then disables itself. Use the
checker directly without those switches for read-only polling. It disables on
red validation too, so after fixing a red run, reinstall or re-enable the task.
Before posting any ping, check the PR comments for the `winget-review-ping`
marker and do not add a duplicate.

For a machine-independent merge check, the repo's
`.github/workflows/winget-publish-watch.yml` runs every 15 minutes, confirms
the upstream PR is merged and the versioned manifest exists on winget-pkgs
master, then disables its own schedule. CI cannot approve or merge the PR; the
local `VolumeControl-WingetMergeWatch` task still verifies the Windows client
catalog with `winget show`.

The GitHub App reports validation through PR labels and comments. Do not rely
on `gh pr checks` alone; its check-run list may only show CLA while the
manifest pipeline is still running. Read labels:

- `New-Package` / `New-Manifest` means review state, not validation result.
- `Validation-Completed` plus `Azure-Pipeline-Passed` is green.
- `*Error`, `Needs-*`, `Blocking-Issue`, `Policy-Test`, `Validation-Domain`,
  `Validation-Executable-Error` and similar labels require action.

On a red label, read the bot comment and fix the named step. Do not infer the
failure from a missing `Validation-Completed`. A moderator can re-trigger with
`@wingetbot run` after a transient failure or fix; contributors cannot.
Validation often takes 35-40 minutes; longer can mean pipeline delay, not a
manifest defect. Local `winget validate` takes milliseconds and cannot prove
upstream success.

## 5. Moderator review

Wait for `Validation-Completed` before asking for review. The new-package queue
is long and is not a strict FIFO; validation, policy, and package complexity
change order.

Check who is actively approving recently merged PRs, then mention at most two
moderators with a short note:

```powershell
gh api 'search/issues?q=repo:microsoft/winget-pkgs+is:pr+is:merged+label:Moderator-Approved+sort:updated-desc&per_page=20' `
  --jq '.items[].number'
```

Look at the `Moderator-Approved` timeline event or APPROVED review to find the
human moderator. Examples observed in this repo's history: `ImJoakim`,
`Trenly`. Verify current activity; do not mass-tag. A short comment is enough:
"PR #... is validation-green and ready for review: new package, local install
evidence posted." Use the Matrix room `#Microsoft_winget-pkgs:gitter.im` for a
status question if you have an account; GitHub mentions are the fallback.

## 6. After merge and publication

Approval applies `Moderator-Approved`; the bot auto-merges, then the publish
pipeline runs. Changes usually appear in the source within one hour.

```powershell
gh pr view <number> -R microsoft/winget-pkgs --json state,mergedAt,url
winget source update
winget show --id dt418.VolumeControl --exact
winget install --id dt418.VolumeControl --exact --version 0.1.10
```

Only call the package published after the catalog shows the version and a
catalog-based install succeeds. Record the automation/skill/CI landing in
`feature_list.json` and `claude-progress.md` immediately; add the publication
result and update the landing page's winget note only after the catalog
verification. Clean up the fork branch/worktree; keep the submitted
`winget/<version>/` snapshot.

## Common mistakes

- Running `winget validate --manifest .\winget\...` from `C:\Users\<user>`.
  The path is relative to the current directory; install and use
  `winget-manifest` instead.
- Mixing shells: PowerShell wants `.\scripts\...`; Git Bash wants
  `./scripts/...`. Backslashes disappear in Bash (`.\scripts` becomes
  `.scripts`). The global `winget-*` commands avoid both problems.
- Running `Enable-WindowsOptionalFeature` in PowerShell 7 and getting
  `Class not registered`. Use elevated Windows PowerShell 5.1 or
  `optionalfeatures.exe`.
- Pushing a commit during validation, which resets the pipeline.
- Opening a second PR instead of updating the existing unmerged new-package
  PR.
- Deleting an old version directory from a PR that is already merged.
- Treating local `winget validate`, `gh pr checks`, or a green label as
  publication.
- Using `winget list --id` to disprove a local install; check `--name` and ARP.
- Forgetting the exact installer filename or tag format; the generator
  intentionally refuses mismatches.
