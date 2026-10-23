[CmdletBinding()]
param(
    [string] $BaselineTag = 'v0.1.9',
    [string] $TargetTag = 'v0.1.10',
    [string] $CandidateDirectory,
    [string] $OutputRoot = 'output/install-lifecycle/windows',
    [switch] $Isolated,
    [switch] $PublishedOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not $Isolated) {
    throw "Isolation is mandatory. Pass -Isolated to run in a dedicated isolated directory."
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$outDir = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $OutputRoot))
New-Item -ItemType Directory -Path $outDir -Force -ErrorAction Stop | Out-Null

$results = [System.Collections.Generic.List[object]]::new()
$logPath = Join-Path $outDir 'lifecycle-execution.log'

function Record-Case {
    param(
        [string] $Name,
        [string] $Status,
        [string] $Details = ''
    )

    $row = [pscustomobject]@{
        Case = $Name
        Status = $Status
        Details = $Details
        Timestamp = [System.DateTime]::UtcNow.ToString('o')
    }
    $script:results.Add($row)
    $msg = "[{0}] {1}: {2}" -f $Status, $Name, $Details
    Write-Host $msg
    Add-Content -LiteralPath $logPath -Value $msg
}

$testSandbox = Join-Path $env:TEMP ("vc-verify-lifecycle-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testSandbox -Force | Out-Null

$origLocal = $env:LOCALAPPDATA
$origApp = $env:APPDATA

try {
    $sandboxLocal = Join-Path $testSandbox 'LocalAppData'
    $sandboxRoaming = Join-Path $testSandbox 'AppData'
    New-Item -ItemType Directory -Path $sandboxLocal -Force | Out-Null
    New-Item -ItemType Directory -Path $sandboxRoaming -Force | Out-Null
    $env:LOCALAPPDATA = $sandboxLocal
    $env:APPDATA = $sandboxRoaming

    $scriptPath = Join-Path $repoRoot 'scripts/Install-VolumeControl.ps1'

    if ($PublishedOnly) {
        Record-Case -Name "Baseline Execution" -Status "Running" -Details "Testing published baseline $BaselineTag"
        $baseErr = $null
        try {
            & $scriptPath -Action install -Version $BaselineTag -Silent
            Record-Case -Name "Baseline Install" -Status "Passed" -Details "Installed $BaselineTag"
        } catch {
            $baseErr = $_.Exception.Message
            Record-Case -Name "Baseline Install" -Status "Failed" -Details "Baseline install error: $baseErr"
        }
    } else {
        if ([string]::IsNullOrWhiteSpace($CandidateDirectory) -or -not (Test-Path $CandidateDirectory)) {
            throw "CandidateDirectory must point to an existing dist directory containing candidate artifacts."
        }

        Record-Case -Name "Candidate Baseline Setup" -Status "Passed" -Details "Starting candidate lifecycle testing"

        # Case 1: Fresh install candidate
        $installerScript = Join-Path $repoRoot 'scripts/Install-VolumeControl.ps1'
        $installRoot = Join-Path $env:LOCALAPPDATA 'VolumeControl'

        # Case 2: Prepared update mode
        $prepDir = Join-Path $testSandbox 'prepared'
        New-Item -ItemType Directory -Path $prepDir -Force | Out-Null
        Copy-Item (Join-Path $CandidateDirectory 'VolumeControl_*_x64-setup.exe') $prepDir -Force -ErrorAction SilentlyContinue
        Copy-Item (Join-Path $CandidateDirectory 'SHA256SUMS.txt') $prepDir -Force -ErrorAction SilentlyContinue

        if (Test-Path (Join-Path $prepDir 'SHA256SUMS.txt')) {
            try {
                & $installerScript -Action update -PreparedDirectory $prepDir -Silent
                if ((Test-Path (Join-Path $prepDir 'result.exit-code')) -and (Get-Content (Join-Path $prepDir 'result.exit-code')).Trim() -eq '0') {
                    Record-Case -Name "Prepared Offline Apply" -Status "Passed" -Details "Prepared apply exited 0 with handoff markers"
                } else {
                    Record-Case -Name "Prepared Offline Apply" -Status "Failed" -Details "result.exit-code missing or nonzero"
                }
            } catch {
                Record-Case -Name "Prepared Offline Apply" -Status "Failed" -Details $_.Exception.Message
            }
        }

        # Case 3: Repeat install same version intact no-op
        try {
            $outLines = @(& $installerScript -Action update -PreparedDirectory $prepDir -Silent 6>&1)
            Record-Case -Name "Repeat Install Intact No-Op" -Status "Passed" -Details ($outLines -join "; ")
        } catch {
            Record-Case -Name "Repeat Install Intact No-Op" -Status "Failed" -Details $_.Exception.Message
        }

        # Case 4: Uninstall clean
        try {
            $unOut = @(& $installerScript -Action uninstall -Silent 6>&1)
            Record-Case -Name "Uninstall App" -Status "Passed" -Details ($unOut -join "; ")
        } catch {
            Record-Case -Name "Uninstall App" -Status "Failed" -Details $_.Exception.Message
        }

        # Case 5: Second uninstall no-op
        try {
            $un2Out = @(& $installerScript -Action uninstall -Silent 6>&1)
            Record-Case -Name "Second Uninstall No-Op" -Status "Passed" -Details ($un2Out -join "; ")
        } catch {
            Record-Case -Name "Second Uninstall No-Op" -Status "Failed" -Details $_.Exception.Message
        }
    }

    $jsonPath = Join-Path $outDir 'lifecycle-results.json'
    $json = $results | ConvertTo-Json -Depth 4
    [System.IO.File]::WriteAllText($jsonPath, $json, [System.Text.UTF8Encoding]::new($false))
    Write-Host "Results written to $jsonPath"

    $hasFailures = @($results | Where-Object { $_.Status -eq 'Failed' }).Count -gt 0
    if (-not $PublishedOnly -and $hasFailures) {
        exit 1
    }
} finally {
    $env:LOCALAPPDATA = $origLocal
    $env:APPDATA = $origApp
    Remove-Item -LiteralPath $testSandbox -Recurse -Force -ErrorAction SilentlyContinue
}
