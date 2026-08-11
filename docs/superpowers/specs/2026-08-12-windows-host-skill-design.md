# windows-host Skill Design

Date: 2026-08-12
Status: approved (brainstormed with user, 3 parts approved)

## Purpose

Volume-control development happens on a Windows host. This session hit seven
environment/tooling gotchas that cost real time. This skill packages them so
future sessions (and agents) do not repeat the mistakes: it documents each
gotcha (trigger -> symptom -> fix) and ships a helper script that recreates
the lost pkg-config cross-check stub automatically.

## Approach (chosen)

Single skill `windows-host` + one helper script + self-test wiring:

```
.agents/skills/windows-host/
  SKILL.md
  scripts/ensure-pkg-config-stub.ps1
.claude/skills/windows-host/           # byte-identical mirror
  SKILL.md
  scripts/ensure-pkg-config-stub.ps1
```

Alternative approaches considered and rejected: doc-only skill (no drift
protection), full tooling with sync-mirrors helper + CI changes (too heavy
for now).

## SKILL.md content — seven gotchas

Each gotcha documented as trigger -> symptom -> fix:

1. **Encoding-safe edits** — never `Set-Content -Encoding UTF8` /
   `.Replace` on tracked markdown/JSON: adds BOM and double-encodes
   em-dashes (`—` becomes `â€”`), rewriting the whole file as churn.
   Use the edit tool, or .NET `[IO.File]::WriteAllText` with UTF8
   no-BOM encoding for generated files.
2. **pkg-config stub** — `%TEMP%\rtk-stub-bin\pkg-config.cmd` is the
   environment shim for cross-target checks from Windows. It gets
   deleted; recreate with `scripts/ensure-pkg-config-stub.ps1` and set
   `PKG_CONFIG` + `PKG_CONFIG_ALLOW_CROSS=1` before cross checks. The
   stub MUST print a version line for `--modversion` — a pure exit-0
   stub panics inside pkg-config-0.3.33 `parse_modversion`
   (Option::unwrap on empty output).
3. **bash vs sh** — self-tests must run under Git Bash
   (`bash scripts/...`), never `sh` from PowerShell: MSYS `/tmp` paths
   break native git.exe (`fatal: cannot change to '/tmp/...'`, 9
   spurious FAILs). Never the WSL shim (`System32\bash.exe`).
4. **Mirror sync** — editing `scripts/*.sh|ps1` requires syncing the
   `.agents/skills/<skill>/scripts/` and `.claude/skills/<skill>/scripts/`
   copies byte-identical; the format-lint smoke asserts this and fails
   otherwise.
5. **Stale trees check** — before committing, grep for third skill
   trees and unwired lock files (history: `agent/skills/`, `skills-lock.json`
   both deleted as stale zero-reference artifacts).
6. **Session handoff refresh** — update `session-handoff.md` after every
   substantive landing (session number, feature count, test count,
   self-test counts).
7. **Nested quoting** — calling `bash`/`sh -c` with complex quoting from
   PowerShell is fragile; write temp scripts under
   `C:\Users\<user>\AppData\Local\Temp\opencode\` instead.

## Helper script — ensure-pkg-config-stub.ps1

- Idempotent: creates `%TEMP%\rtk-stub-bin\pkg-config.cmd` if missing;
  verifies it answers `--modversion` with `1.0` if present.
- Writes the stub with UTF-8 no-BOM (`[IO.File]::WriteAllText`), never
  Set-Content — practicing gotcha #1 itself.
- Stub content (verified working against pkg-config-0.3.33):

  ```
  @echo off
  for %%a in (%*) do if "%%a"=="--modversion" echo 1.0
  exit /b 0
  ```

- Prints the stub path + the env vars to set
  (`PKG_CONFIG=...`, `PKG_CONFIG_ALLOW_CROSS=1`).
- Exit 0 = stub ready; exit 1 = failure with message.

## Self-test wiring

- `scripts/test-check-records.sh`: add one check asserting
  `.agents/skills/windows-host` and `.claude/skills/windows-host` are
  byte-identical (mirroring the existing guardrail / pre-push-review
  assertions). Check count 32 -> 33.
- The pre-push-review SKILL.md baseline text and session-handoff.md
  counts must be updated to the new check count in the same change set.

## Verification

- `bash scripts/test-check-records.sh` — 33 checks pass, exit 0.
- `powershell -File scripts/ensure-pkg-config-stub.ps1` — creates/verifies
  the stub, exit 0; run twice (idempotency).
- Cross-check sanity: `cargo check --target x86_64-unknown-linux-gnu -p
  volumectl --tests --no-default-features --features gtk-renderer` with
  stub env — compiles clean.
- `bash scripts/test-format-lint.sh`, `bash scripts/test-ship.sh`,
  both gates, `cargo test` — green.
- `sh scripts/check-records.sh --branch origin/master` and `--staged` on
  the landed set — exit 0.

## Records

- `feature_list.json`: new entry vol-027 (area: tooling) — windows-host
  skill + helper + wiring; verification + evidence.
- `claude-progress.md`: Session 035 entry.
- Both land in the same commit as the skill files (guardrail rule).
- Ship via `scripts/ship.ps1 -Push`.

## Out of scope

- CI changes, sync-mirrors automation, docs/ updates beyond the skill
  files and records.
