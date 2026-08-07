<#
    format-lint.ps1 - deterministic quality gate for volume-control.

    Mirrors the CI checks (see .github/workflows/ci.yml and .githooks/pre-commit)
    so local Windows runs and remote gates agree on command order and the
    -D warnings policy.

    Usage:
      .\format-lint.ps1              # fmt --check, diff --check, clippy, tests
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

$cargo = Get-Cargo
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot

$script:failed = $false

function Invoke-Step {
    param(
        [string]$Name,
        [scriptblock]$Body
    )
    Write-Host "`n[$Name]" -ForegroundColor Cyan
    & $Body
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[$Name] FAILED (exit $LASTEXITCODE)" -ForegroundColor Red
        $script:failed = $true
    } else {
        Write-Host "[$Name] OK" -ForegroundColor Green
    }
}

try {
    # 1. Format (--all covers every workspace member).
    if ($Fix) {
        Invoke-Step 'cargo fmt --all' { & $cargo fmt --all }
    } else {
        Invoke-Step 'cargo fmt --all --check' { & $cargo fmt --all --check }
    }

    # 2. Whitespace / diff hygiene.
    Invoke-Step 'git diff --check' { git diff --check }

    # 3. Clippy with -D warnings (project policy: never weaken this).
    if ($AllFeatures) {
        Invoke-Step 'cargo clippy --workspace --all-targets --all-features -- -D warnings' {
            & $cargo clippy --workspace --all-targets --all-features -- -D warnings
        }
    } else {
        Invoke-Step 'cargo clippy --workspace --all-targets --no-default-features -- -D warnings' {
            & $cargo clippy --workspace --all-targets --no-default-features -- -D warnings
        }
    }

    # 4. Tests.
    if (-not $SkipTests) {
        if ($AllFeatures) {
            Invoke-Step 'cargo test --workspace --all-features' {
                & $cargo test --workspace --all-features
            }
        } else {
            Invoke-Step 'cargo test --workspace --no-default-features' {
                & $cargo test --workspace --no-default-features
            }
        }
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
