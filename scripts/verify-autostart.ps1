#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Verifies the Windows current-user VolumeControl auto-start registration.

.DESCRIPTION
    This is a diagnostic-only registry check. It never starts the supplied
    binary: the current Tauri host exposes auto-start through IPC, while the
    adapter's durable contract is the HKCU Run value. The binary is used only
    as a trusted, workspace-local executable path when constructing the
    expected command.

    The VolumeControl value is snapshotted before any write. Enable, read
    back, disable, and restore all happen inside try/finally cleanup. Only the
    VolumeControl value is written or deleted; unrelated Run values are
    fingerprinted and verified unchanged.

.EXAMPLE
    pwsh -NoProfile -File scripts/verify-autostart.ps1
      -Binary target/release/VolumeControl.exe
      -OutputRoot output/tauri-surface-evidence/autostart
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Binary,
    [string]$OutputRoot = 'output/tauri-surface-evidence/autostart'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$runSubKey = 'Software\Microsoft\Windows\CurrentVersion\Run'
$runValueName = 'VolumeControl'
$outputPath = $null
$jsonPath = $null
$failurePath = $null
$binaryPath = $null
$expectedCommand = $null
$originalState = $null
$mutationAttempted = $false
$exitCode = 0
$failureMessage = $null
$evidence = [ordered]@{
    verifier = 'verify-autostart.ps1'
    started_at_utc = (Get-Date).ToUniversalTime().ToString('o')
    platform = [Environment]::OSVersion.Platform.ToString()
    run_key = "HKCU\$runSubKey"
    value_name = $runValueName
    adapter = 'windows-registry-direct'
    binary_launched = $false
    binary = $null
    binary_sha256 = $null
    expected_command = $null
    original = $null
    enable = $null
    disable = $null
    restore = $null
    unrelated_run_values_unchanged = $null
    status = 'running'
    error = $null
}

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Text
    )
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($Path, $Text, $encoding)
}

function Get-Sha256Text {
    param([AllowNull()][string]$Text)
    if ($null -eq $Text) {
        $Text = '<null>'
    }
    $bytes = [Text.Encoding]::UTF8.GetBytes($Text)
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
    } finally {
        $sha.Dispose()
    }
}

function Convert-RegistryValueForFingerprint {
    param(
        [AllowNull()][object]$Value,
        [Parameter(Mandatory = $true)]
        [Microsoft.Win32.RegistryValueKind]$Kind
    )
    if ($null -eq $Value) {
        return "$Kind|<null>"
    }
    if ($Value -is [byte[]]) {
        return "$Kind|bytes:$([Convert]::ToBase64String($Value))"
    }
    if ($Value -is [Array]) {
        $parts = @($Value | ForEach-Object { [string]$_ })
        return "$Kind|array:$($parts -join [char]0)"
    }
    return "$Kind|$([string]$Value)"
}

function Get-RegistryValueFingerprint {
    param(
        [AllowNull()][object]$Value,
        [Parameter(Mandatory = $true)]
        [Microsoft.Win32.RegistryValueKind]$Kind
    )
    return Get-Sha256Text (Convert-RegistryValueForFingerprint $Value $Kind)
}

function Open-RunRegistryKey {
    param([bool]$Writable)

    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey(
        [Microsoft.Win32.RegistryHive]::CurrentUser,
        [Microsoft.Win32.RegistryView]::Default
    )
    try {
        $key = $base.OpenSubKey($runSubKey, $Writable)
        if ($null -eq $key) {
            $base.Dispose()
            return $null
        }
        return [pscustomobject]@{
            Base = $base
            Key = $key
        }
    } catch {
        $base.Dispose()
        throw
    }
}

function Close-RunRegistryKey {
    param([AllowNull()][object]$Handle)
    if ($null -eq $Handle) {
        return
    }
    try {
        if ($null -ne $Handle.Key) {
            $Handle.Key.Dispose()
        }
    } finally {
        if ($null -ne $Handle.Base) {
            $Handle.Base.Dispose()
        }
    }
}

function Get-RunState {
    $handle = Open-RunRegistryKey $false
    if ($null -eq $handle) {
        return [pscustomobject]@{
            key_exists = $false
            volume_exists = $false
            volume_kind = $null
            volume_value = $null
            volume_fingerprint = $null
            unrelated_fingerprints = @{}
            unrelated_names = @()
        }
    }

    try {
        $key = $handle.Key
        $names = @($key.GetValueNames())
        $unrelatedFingerprints = @{}
        $unrelatedNames = @()
        foreach ($name in $names) {
            if ($name -eq $runValueName) {
                continue
            }
            $kind = $key.GetValueKind($name)
            $value = $key.GetValue(
                $name,
                $null,
                [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
            )
            $unrelatedNames += $name
            $unrelatedFingerprints[$name] = Get-RegistryValueFingerprint $value $kind
        }

        $volumeExists = $names -contains $runValueName
        $volumeKind = $null
        $volumeValue = $null
        $volumeFingerprint = $null
        if ($volumeExists) {
            $volumeKind = $key.GetValueKind($runValueName)
            $volumeValue = $key.GetValue(
                $runValueName,
                $null,
                [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
            )
            $volumeFingerprint = Get-RegistryValueFingerprint $volumeValue $volumeKind
        }

        return [pscustomobject]@{
            key_exists = $true
            volume_exists = $volumeExists
            volume_kind = $volumeKind
            volume_value = $volumeValue
            volume_fingerprint = $volumeFingerprint
            unrelated_fingerprints = $unrelatedFingerprints
            unrelated_names = @($unrelatedNames)
        }
    } finally {
        Close-RunRegistryKey $handle
    }
}

function Convert-StateForEvidence {
    param([Parameter(Mandatory = $true)][object]$State)
    return [ordered]@{
        key_exists = [bool]$State.key_exists
        volume_exists = [bool]$State.volume_exists
        volume_kind = if ($null -eq $State.volume_kind) { $null } else { [string]$State.volume_kind }
        volume_fingerprint = $State.volume_fingerprint
        unrelated_names = @($State.unrelated_names | Sort-Object)
    }
}

function Test-UnrelatedValuesUnchanged {
    param(
        [Parameter(Mandatory = $true)][object]$Before,
        [Parameter(Mandatory = $true)][object]$After
    )
    $beforeNames = @($Before.unrelated_fingerprints.Keys | Sort-Object)
    $afterNames = @($After.unrelated_fingerprints.Keys | Sort-Object)
    if ($beforeNames.Count -ne $afterNames.Count) {
        return $false
    }
    for ($index = 0; $index -lt $beforeNames.Count; $index++) {
        if ([string]$beforeNames[$index] -cne [string]$afterNames[$index]) {
            return $false
        }
    }
    foreach ($name in $beforeNames) {
        if ($Before.unrelated_fingerprints[$name] -cne $After.unrelated_fingerprints[$name]) {
            return $false
        }
    }
    return $true
}

function Set-RunValue {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )
    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey(
        [Microsoft.Win32.RegistryHive]::CurrentUser,
        [Microsoft.Win32.RegistryView]::Default
    )
    $key = $null
    try {
        $key = $base.OpenSubKey($runSubKey, $true)
        if ($null -eq $key) {
            $key = $base.CreateSubKey($runSubKey, $true)
        }
        if ($null -eq $key) {
            throw "Could not open or create HKCU\$runSubKey for writing."
        }
        $key.SetValue($runValueName, $Value, [Microsoft.Win32.RegistryValueKind]::String)
    } finally {
        if ($null -ne $key) {
            $key.Dispose()
        }
        $base.Dispose()
    }
}

function Remove-RunValue {
    $handle = Open-RunRegistryKey $true
    if ($null -eq $handle) {
        return
    }
    try {
        if ($handle.Key.GetValueNames() -contains $runValueName) {
            $handle.Key.DeleteValue($runValueName, $false)
        }
    } finally {
        Close-RunRegistryKey $handle
    }
}

function Restore-OriginalRunValue {
    param([Parameter(Mandatory = $true)][object]$State)
    if (-not $State.volume_exists) {
        Remove-RunValue
        return
    }

    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey(
        [Microsoft.Win32.RegistryHive]::CurrentUser,
        [Microsoft.Win32.RegistryView]::Default
    )
    $key = $null
    try {
        $key = $base.OpenSubKey($runSubKey, $true)
        if ($null -eq $key) {
            $key = $base.CreateSubKey($runSubKey, $true)
        }
        if ($null -eq $key) {
            throw "Could not reopen HKCU\$runSubKey while restoring $runValueName."
        }
        $key.SetValue($runValueName, $State.volume_value, $State.volume_kind)
    } finally {
        if ($null -ne $key) {
            $key.Dispose()
        }
        $base.Dispose()
    }
}

function Resolve-TrustedBinary {
    param([Parameter(Mandatory = $true)][string]$Path)
    $candidate = if ([IO.Path]::IsPathRooted($Path)) {
        [IO.Path]::GetFullPath($Path)
    } else {
        [IO.Path]::GetFullPath((Join-Path $repoRoot $Path))
    }
    if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
        throw "Binary not found: $candidate"
    }
    $resolved = (Resolve-Path -LiteralPath $candidate).ProviderPath
    $repoPrefix = $repoRoot.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($repoPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to use a binary outside the repository workspace: $resolved"
    }
    if ([IO.Path]::GetFileName($resolved) -ine 'VolumeControl.exe') {
        throw "Refusing unexpected binary name '$([IO.Path]::GetFileName($resolved))'; expected VolumeControl.exe."
    }
    return $resolved
}

function Resolve-OutputPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    if ([IO.Path]::IsPathRooted($Path)) {
        return [IO.Path]::GetFullPath($Path)
    }
    return [IO.Path]::GetFullPath((Join-Path $repoRoot $Path))
}

function Get-ExpectedCommand {
    param([Parameter(Mandatory = $true)][string]$Path)
    # VolumeControl.exe cannot contain a quote in a valid Windows file name.
    # The adapter stores the executable as one quoted command-line argument.
    return '"' + $Path + '"'
}

function Test-ExpectedCommand {
    param(
        [Parameter(Mandatory = $true)][object]$State,
        [Parameter(Mandatory = $true)][string]$Command
    )
    return $State.volume_exists -and
        $State.volume_kind -eq [Microsoft.Win32.RegistryValueKind]::String -and
        ([string]$State.volume_value).Equals($Command, [StringComparison]::OrdinalIgnoreCase)
}

function Test-RestoredState {
    param(
        [Parameter(Mandatory = $true)][object]$Expected,
        [Parameter(Mandatory = $true)][object]$Actual
    )
    if ([bool]$Expected.volume_exists -ne [bool]$Actual.volume_exists) {
        return $false
    }
    if (-not $Expected.volume_exists) {
        return -not $Actual.volume_exists
    }
    return $Expected.volume_kind -eq $Actual.volume_kind -and
        $Expected.volume_fingerprint -eq $Actual.volume_fingerprint
}

function Save-Evidence {
    $evidence.finished_at_utc = (Get-Date).ToUniversalTime().ToString('o')
    $json = $evidence | ConvertTo-Json -Depth 12
    Write-Utf8NoBom $jsonPath $json
    if ($null -ne $failureMessage) {
        Write-Utf8NoBom $failurePath $failureMessage
    }
}

try {
    $outputPath = Resolve-OutputPath $OutputRoot
    New-Item -ItemType Directory -Force -Path $outputPath | Out-Null
    $jsonPath = Join-Path $outputPath 'autostart.json'
    $failurePath = Join-Path $outputPath 'failure.txt'

    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        throw 'verify-autostart.ps1 is Windows-only; no registry was changed and no binary was started.'
    }

    $binaryPath = Resolve-TrustedBinary $Binary
    $expectedCommand = Get-ExpectedCommand $binaryPath
    $evidence.binary = $binaryPath
    $evidence.binary_sha256 = (Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $evidence.expected_command = $expectedCommand
    $originalState = Get-RunState
    $evidence.original = Convert-StateForEvidence $originalState

    $mutationAttempted = $true
    Set-RunValue $expectedCommand
    $enabledState = Get-RunState
    if (-not (Test-ExpectedCommand $enabledState $expectedCommand)) {
        throw "Enable read-back mismatch: expected REG_SZ command $expectedCommand."
    }
    $evidence.enable = [ordered]@{
        passed = $true
        read_back_command = [string]$enabledState.volume_value
        command_fingerprint = $enabledState.volume_fingerprint
        state = Convert-StateForEvidence $enabledState
    }

    Remove-RunValue
    $disabledState = Get-RunState
    if ($disabledState.volume_exists) {
        throw "Disable verification failed: HKCU\$runSubKey\$runValueName still exists."
    }
    $evidence.disable = [ordered]@{
        passed = $true
        state = Convert-StateForEvidence $disabledState
    }
    $evidence.unrelated_run_values_unchanged = Test-UnrelatedValuesUnchanged $originalState $disabledState
    if (-not $evidence.unrelated_run_values_unchanged) {
        throw 'An unrelated HKCU Run value changed during the diagnostic.'
    }

    $evidence.status = 'passed'
    Write-Host "PASS auto-start registry enable/read-back/disable completed for $binaryPath"
} catch {
    $exitCode = 1
    $failureMessage = $_.Exception.Message
    $evidence.status = 'failed'
    $evidence.error = $failureMessage
    Write-Error $failureMessage
} finally {
    if ($null -ne $originalState -and $mutationAttempted) {
        try {
            Restore-OriginalRunValue $originalState
            $restoredState = Get-RunState
            $restorePassed = Test-RestoredState $originalState $restoredState
            $unrelatedRestored = Test-UnrelatedValuesUnchanged $originalState $restoredState
            $evidence.restore = [ordered]@{
                attempted = $true
                passed = $restorePassed -and $unrelatedRestored
                state = Convert-StateForEvidence $restoredState
                unrelated_run_values_unchanged = $unrelatedRestored
            }
            if (-not $restorePassed -or -not $unrelatedRestored) {
                $exitCode = 1
                $evidence.status = 'failed'
                $failureMessage = if ($null -ne $failureMessage) {
                    "$failureMessage; registry restoration verification failed."
                } else {
                    'Registry restoration verification failed.'
                }
                $evidence.error = $failureMessage
            }
        } catch {
            $exitCode = 1
            $evidence.status = 'failed'
            $failureMessage = if ($null -ne $failureMessage) {
                "$failureMessage; registry restoration failed: $($_.Exception.Message)"
            } else {
                "Registry restoration failed: $($_.Exception.Message)"
            }
            $evidence.error = $failureMessage
        }
    } elseif ($null -eq $originalState) {
        $evidence.restore = [ordered]@{
            attempted = $false
            passed = $true
            reason = 'No registry mutation was attempted.'
        }
    }

    if ($null -ne $jsonPath) {
        try {
            Save-Evidence
        } catch {
            $exitCode = 1
            Write-Error "Could not write auto-start evidence: $($_.Exception.Message)"
        }
    }
}

exit $exitCode
