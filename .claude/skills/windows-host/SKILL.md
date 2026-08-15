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
- Symptom: session-handoff.md shows an older session number, feature count,
  or test count, and the next session trusts the stale numbers.
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
bash scripts/check-records.sh --branch origin/main
powershell -ExecutionPolicy Bypass -File scripts/ship.ps1 -Push
```

## Before commit checklist

- [ ] No PowerShell Set-Content on tracked .md/.json this session
- [ ] Skill mirrors byte-identical (`cmp -s` or hash check)
- [ ] Self-tests run under bash, exit 0
- [ ] Index clean before ship / test-format-lint.sh (the smoke needs an empty
      staged set; `git reset` if you staged anything)
- [ ] session-handoff.md refreshed
- [ ] feature_list.json + claude-progress.md updated in the same commit
