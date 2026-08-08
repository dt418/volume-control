<#
    ship.ps1 - mandatory pre-ship flow for volume-control (PowerShell).

    Thin native wrapper over scripts/ship.sh: it resolves the same Git for
    Windows bash the format-lint PowerShell gate uses and invokes the
    canonical bash flow with the mapped flags. The rule logic lives ONLY in
    ship.sh, so the two entry points cannot drift (the same bridge pattern
    the format-lint gate uses for its record-updates step).

    Usage:
      .\scripts\ship.ps1                  # verify, stage, commit (no push)
      .\scripts\ship.ps1 -Push            # verify, stage, commit, push
      .\scripts\ship.ps1 -DryRun          # verify, change nothing
      .\scripts\ship.ps1 -Force           # relax soft preconditions (hygiene only)
      .\scripts\ship.ps1 -Message "..."   # commit message

    Exit code 0 = ok; 1 = a hard check or commit/push failed; 2 = bad usage.
#>
param(
    [switch]$Push,
    [switch]$Force,
    [switch]$DryRun,
    [string]$Message
)

$ErrorActionPreference = 'Stop'

# -- Git resolution (mirrors format-lint.ps1's Get-Git; keep in sync) --------
function Get-Git {
    $fromPath = Get-Command git -ErrorAction SilentlyContinue
    if ($fromPath) { return $fromPath.Source }
    $locations = @()
    foreach ($base in @($env:ProgramFiles, ${env:ProgramFiles(x86)})) {
        if ($base) { $locations += (Join-Path $base 'Git\cmd\git.exe') }
    }
    foreach ($candidate in $locations) {
        if (Test-Path -LiteralPath $candidate) { return $candidate }
    }
    throw "git not found on PATH and not at a standard Git for Windows location"
}

# -- Bash resolution (mirrors format-lint.ps1's Get-Bash; keep in sync) ------
# Prefer the bash that belongs to the same Git for Windows install as git
# (walk up looking for a bin\bash.exe), then standard install locations, then
# a PATH bash -- but NEVER the Windows Subsystem for Linux shim
# (System32\bash.exe), which would run the flow under WSL's Linux git.
function Get-Bash {
    $dir = Split-Path -Parent $git
    $hops = 0
    while ($dir -and $hops -lt 5) {
        $candidate = Join-Path $dir 'bin\bash.exe'
        if (Test-Path -LiteralPath $candidate) { return $candidate }
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
        if (Test-Path -LiteralPath $candidate) { return $candidate }
    }
    $fromPath = Get-Command bash -ErrorAction SilentlyContinue
    if ($fromPath) {
        $wslShim = Join-Path $env:SystemRoot 'System32\bash.exe'
        if ($fromPath.Source -ne $wslShim) { return $fromPath.Source }
    }
    return $null
}

$git = Get-Git
$bash = Get-Bash
if (-not $bash) {
    Write-Host 'ship.ps1 needs Git Bash (Git for Windows) to run scripts/ship.sh' -ForegroundColor Red
    exit 1
}

# -- Repository root (walk up to the workspace Cargo.toml) --------------------
$dir = $PSScriptRoot
while ($dir) {
    if (Test-Path -LiteralPath (Join-Path $dir 'Cargo.toml')) { break }
    $parent = Split-Path -Parent $dir
    if ($parent -eq $dir) {
        throw "could not locate the workspace root (no Cargo.toml) above $PSScriptRoot"
    }
    $dir = $parent
}
$repoRoot = $dir

# -- Map flags and delegate to the canonical flow -------------------------------
$shipArgs = @()
if ($Push)   { $shipArgs += '--push' }
if ($Force)  { $shipArgs += '--force' }
if ($DryRun) { $shipArgs += '--dry-run' }
if ($Message) { $shipArgs += '--message'; $shipArgs += $Message }

$code = 1
Push-Location $repoRoot
try {
    & $bash 'scripts/ship.sh' @shipArgs
    if ($null -ne $LASTEXITCODE) { $code = $LASTEXITCODE }
} finally {
    Pop-Location
}
exit $code
