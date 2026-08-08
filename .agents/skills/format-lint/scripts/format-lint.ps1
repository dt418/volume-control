<#
    format-lint.ps1 - deterministic quality gate for volume-control.

    Mirrors the CI checks (see .github/workflows/ci.yml and .githooks/pre-commit)
    and the bash gate (scripts/format-lint.sh). The step list is defined ONCE
    in scripts/format-lint-steps.json and both gates execute it, so the two
    implementations cannot drift. Flags only transform the manifest's default
    steps (see SKILL.md).

    Usage:
      .\format-lint.ps1              # fmt --check, diff checks, clippy, tests
      .\format-lint.ps1 -Fix         # apply `cargo fmt --all`, then the full gate
      .\format-lint.ps1 -SkipTests   # format/lint only
      .\format-lint.ps1 -AllFeatures # include gtk-renderer/layer-shell (needs GTK dev libs)

    Exit code 0 = gate passed; 1 = a step failed.
#>
param(
    [switch]$Fix,
    [switch]$SkipTests,
    [switch]$AllFeatures
)

$ErrorActionPreference = 'Stop'

# -- Rust toolchain resolution ----------------------------------------------
function Get-Cargo {
    $fromPath = Get-Command cargo -ErrorAction SilentlyContinue
    if ($fromPath) {
        return $fromPath.Source
    }
    $fallback = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
    if (Test-Path -LiteralPath $fallback) {
        return $fallback
    }
    throw "cargo not found on PATH and not at $fallback"
}

# -- Git resolution ---------------------------------------------------------
# Git for Windows is the supported git on Windows; fall back to the standard
# install locations when PATH is stale. Fail loudly rather than letting a
# step die on an unhelpful 'git is not recognized' terminating error.
function Get-Git {
    $fromPath = Get-Command git -ErrorAction SilentlyContinue
    if ($fromPath) {
        return $fromPath.Source
    }
    $locations = @()
    foreach ($base in @($env:ProgramFiles, ${env:ProgramFiles(x86)})) {
        if ($base) { $locations += (Join-Path $base 'Git\cmd\git.exe') }
    }
    foreach ($candidate in $locations) {
        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
    }
    throw "git not found on PATH and not at a standard Git for Windows location"
}

# -- Bash resolution --------------------------------------------------------
# The record-keeping step delegates to scripts/check-records.sh (the single
# implementation of the rule) via bash. Git for Windows ships bash at
# Git\bin\bash.exe, so resolve it the same way as git. Resolved lazily inside
# Invoke-RecordUpdates (not at startup) so a bash-less machine can still run
# the rest of the gate; the records step then fails loudly instead of being
# silently skipped.
function Get-Bash {
    # Prefer the bash that belongs to the same Git for Windows install as
    # $git. git.exe can live in Git\cmd, Git\bin, or Git\mingw64\bin, while
    # bash.exe consistently lives at Git\bin\bash.exe (and Git\usr\bin\bash
    # .exe), so walk up from git.exe's directory looking for a bin\bash.exe.
    # Only then fall back to the standard install locations and a PATH bash
    # -- but NEVER the Windows Subsystem for Linux shim (System32\bash.exe),
    # which would run the records script under WSL's Linux git instead of the
    # repo's Windows git and fail in confusing ways. A PATH bash from any
    # other source (e.g. MSYS2/Cygwin) is accepted as a last resort.
    $dir = Split-Path -Parent $git
    $hops = 0
    while ($dir -and $hops -lt 5) {
        $candidate = Join-Path $dir 'bin\bash.exe'
        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
        $parent = Split-Path -Parent $dir
        if ($parent -eq $dir) { break }
        $dir = $parent
        $hops = $hops + 1
    }
    $locations = @()
    foreach ($base in @($env:ProgramFiles, ${env:ProgramFiles(x86)})) {
        if ($base) { $locations += (Join-Path $base 'Git\bin\bash.exe') }
    }
    foreach ($candidate in $locations) {
        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
    }
    $fromPath = Get-Command bash -ErrorAction SilentlyContinue
    if ($fromPath) {
        $wslShim = Join-Path $env:SystemRoot 'System32\bash.exe'
        if ($fromPath.Source -ne $wslShim) {
            return $fromPath.Source
        }
    }
    return $null
}

# -- Repository root resolution ---------------------------------------------
# Walk up from the script until the workspace Cargo.toml is found, so the gate
# always runs from the repository root no matter where the skill is vendored.
#
# Step authoring notes:
# - $LASTEXITCODE is runspace-global and is NOT reset by PowerShell cmdlets.
#   In any multi-command step, capture $LASTEXITCODE immediately after each
#   native command; a trailing Where-Object/Write-Host can otherwise hide (or
#   inherit) a native failure. Single-command steps may rely on Invoke-Step's
#   own $LASTEXITCODE check instead.
function Get-RepoRoot {
    param([string]$Start)
    $dir = $Start
    while ($dir) {
        if (Test-Path -LiteralPath (Join-Path $dir 'Cargo.toml')) {
            return $dir
        }
        $parent = Split-Path -Parent $dir
        if ($parent -eq $dir) { break }
        $dir = $parent
    }
    throw "could not locate the workspace root (no Cargo.toml) above $Start"
}

$cargo = Get-Cargo
$git = Get-Git
$repoRoot = Get-RepoRoot -Start $PSScriptRoot

# -- Step manifest (single source of truth) -----------------------------------
# JSON parsed natively; keep scripts/format-lint-steps.json and the bash gate's
# line-oriented reader in lockstep.
$manifestPath = Join-Path $repoRoot 'scripts\format-lint-steps.json'
if (-not (Test-Path -LiteralPath $manifestPath)) {
    throw "step manifest not found at $manifestPath"
}
$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
if ($manifest.version -ne 3) {
    throw "unsupported manifest version $($manifest.version) (expected 3)"
}
$steps = @($manifest.steps)
if ($steps.Count -eq 0) {
    throw "no steps found in $manifestPath"
}
$forbiddenPatterns = @($manifest.forbidden_patterns)
if ($forbiddenPatterns.Count -eq 0) {
    throw "no forbidden_patterns found in $manifestPath"
}
$forbiddenPattern = ($forbiddenPatterns -join '|')

Push-Location $repoRoot

$script:failed = $false
$script:stepFailed = $false

function Invoke-Step {
    param(
        [string]$Name,
        [scriptblock]$Body
    )
    $script:stepFailed = $false
    Write-Host "`n[$Name]" -ForegroundColor Cyan
    & $Body
    # A step fails when its body set $script:stepFailed explicitly (multi-step
    # bodies) or when the last native command exited non-zero. $LASTEXITCODE is
    # $null when no native command ran, and $null -ne 0 is $true: fail closed.
    if ($script:stepFailed -or ($LASTEXITCODE -ne 0)) {
        if ($script:stepFailed -and $LASTEXITCODE -eq 0) {
            Write-Host "[$Name] FAILED" -ForegroundColor Red
        } else {
            Write-Host "[$Name] FAILED (exit $LASTEXITCODE)" -ForegroundColor Red
        }
        $script:failed = $true
    } else {
        Write-Host "[$Name] OK" -ForegroundColor Green
    }
}

# Record-keeping guard: reject a commit that changes substantive files
# without updating both records. Delegates to the single implementation in
# scripts/check-records.sh --staged via bash (no duplicated rule logic here).
function Invoke-RecordUpdates {
    $bashPath = Get-Bash
    if (-not $bashPath) {
        Write-Host '  bash not found; the record-updates step needs Git Bash (Git for Windows)' -ForegroundColor Red
        $script:stepFailed = $true
        return
    }
    & $bashPath scripts/check-records.sh --staged
}

# Local mirror of scripts/ci-diff-check.sh: reject forbidden paths in the
# working diff (tracked edits and untracked additions).
function Invoke-ForbiddenPaths {
    # Pattern list comes from the manifest (single source of truth), joined
    # into one alternation; each pattern carries its own anchors.
    $pattern = $forbiddenPattern
    # Multi-command step: capture native exit codes immediately (see step
    # authoring notes) before any cmdlet can mask or inherit them.
    $gitFailed = $false
    $tracked = & $git diff HEAD --name-only --diff-filter=ACMR
    if ($LASTEXITCODE -ne 0) { $gitFailed = $true }
    $untracked = & $git ls-files --others --exclude-standard
    if ($LASTEXITCODE -ne 0) { $gitFailed = $true }
    $tracked = $tracked | Where-Object { $_ -match $pattern }
    $untracked = $untracked | Where-Object { $_ -match $pattern }
    if ($gitFailed) {
        Write-Host '  git command failed while listing diff paths' -ForegroundColor Red
        $script:stepFailed = $true
    } elseif ($tracked -or $untracked) {
        Write-Host '  forbidden paths in the working diff:' -ForegroundColor Red
        @($tracked; $untracked) | Where-Object { $_ } |
            ForEach-Object { Write-Host "  $_" -ForegroundColor Red }
        $script:stepFailed = $true
    }
}

try {
    foreach ($step in $steps) {
        $stepName = [string]$step.name
        $stepId = [string]$step.id
        $skipWhen = [string]$step.skip_when
        $stepArgs = @($step.args)
        $isInternal = $null -ne $step.internal

        # Flag transforms on the displayed name.
        if ($stepId -eq 'fmt' -and $Fix) {
            $stepName = $stepName -replace ' --check$', ''
            $stepArgs = @($stepArgs | Where-Object { $_ -ne '--check' })
        } elseif (($stepId -eq 'clippy' -or $stepId -eq 'test') -and $AllFeatures) {
            $stepName = $stepName -replace '--no-default-features', '--all-features'
            $stepArgs = @($stepArgs | ForEach-Object {
                if ($_ -eq '--no-default-features') { '--all-features' } else { $_ }
            })
        }

        if ($skipWhen -and $skipWhen -ne 'skip_tests') {
            throw "unknown skip_when '$skipWhen' in manifest step '$stepId'"
        }
        if ($skipWhen -eq 'skip_tests' -and $SkipTests) { continue }

        if ($isInternal) {
            $internalId = [string]$step.internal
            switch ($internalId) {
                'forbidden_paths' { Invoke-Step $stepName { Invoke-ForbiddenPaths } }
                'record_updates'  { Invoke-Step $stepName { Invoke-RecordUpdates } }
                default { throw "unknown internal step '$internalId' in manifest step '$stepId'" }
            }
            continue
        }

        $tool = switch ($stepArgs[0]) {
            'cargo' { $cargo }
            'git'   { $git }
            default { throw "unknown tool '$($stepArgs[0])' in manifest step '$stepId'" }
        }
        # Guard the slice: `1..0` would reverse into args[1..0] on a 1-arg step.
        if ($stepArgs.Count -gt 1) {
            $cmdArgs = @($stepArgs[1..($stepArgs.Count - 1)])
        } else {
            $cmdArgs = @()
        }
        Invoke-Step $stepName { & $tool @cmdArgs }
    }

    if ($script:failed) {
        Write-Host "`nGate FAILED - fix the reported step before committing." -ForegroundColor Red
        exit 1
    }
    Write-Host "`nGate passed." -ForegroundColor Green
    exit 0
} finally {
    Pop-Location
}
