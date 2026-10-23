#!/usr/bin/env powershell
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$bootstrap = Join-Path $PSScriptRoot 'Install-VolumeControl.ps1'
$testRoot = Join-Path $env:TEMP ("VolumeControl-bootstrap-test-" + [guid]::NewGuid().ToString('N'))
$originalLocalAppData = $env:LOCALAPPDATA
$originalAppData = $env:APPDATA
$originalStartProcess = Get-Command Start-Process -CommandType Function -ErrorAction SilentlyContinue
$originalGetProcess = Get-Command Get-Process -CommandType Function -ErrorAction SilentlyContinue
$originalStopProcess = Get-Command Stop-Process -CommandType Function -ErrorAction SilentlyContinue
$originalGetItem = Get-Command Get-Item -CommandType Function -ErrorAction SilentlyContinue
$originalVolumeControlLang = $env:VOLUMECONTROL_LANG

function Assert-True {
    param([bool] $Condition, [string] $Message)

    if (-not $Condition) {
        throw "assertion failed: $Message"
    }
}

function Assert-False {
    param([bool] $Condition, [string] $Message = 'expected false')

    if ($Condition) {
        throw "assertion failed: $Message"
    }
}

function Assert-Equal {
    param($Actual, $Expected, [string] $Message)

    if ($Actual -cne $Expected) {
        throw "assertion failed: $Message; expected '$Expected', got '$Actual'"
    }
}

function Assert-Throws {
    param([scriptblock] $Action, [string] $ExpectedMessage)

    try {
        & $Action
    } catch {
        if ($_.Exception.Message -notlike "*$ExpectedMessage*") {
            throw "expected error containing '$ExpectedMessage', got '$($_.Exception.Message)'"
        }
        return
    }

    throw "expected error containing '$ExpectedMessage'"
}

try {
    New-Item -ItemType Directory -Path $testRoot | Out-Null
    $env:LOCALAPPDATA = $testRoot
    . $bootstrap

    Remove-Item Env:\VOLUMECONTROL_LANG -ErrorAction SilentlyContinue
    $expectedDefaultLanguage = 'en'
    if ([System.Globalization.CultureInfo]::CurrentUICulture.TwoLetterISOLanguageName -ceq 'vi') {
        $expectedDefaultLanguage = 'vi'
    }
    Assert-Equal (Get-VolumeControlLanguage) $expectedDefaultLanguage 'language follows the UI culture without an override'
    Assert-Equal (Get-VolumeControlMessage 'install-root.localappdata-required') 'LOCALAPPDATA is required to determine the install root' 'EN template stays byte-identical'
    Assert-Equal (Get-VolumeControlMessage 'install-root.must-equal' 'C:\root') 'install root must equal C:\root' 'EN template interpolates values'
    foreach ($messageId in $script:VolumeControlMessages.Keys) {
        $messageEntry = $script:VolumeControlMessages[$messageId]
        Assert-True (-not [string]::IsNullOrWhiteSpace($messageEntry['en'])) "EN template missing for $messageId"
        Assert-True (-not [string]::IsNullOrWhiteSpace($messageEntry['vi'])) "VI template missing for $messageId"
        Assert-False ($messageEntry['en'] -ceq $messageEntry['vi']) "VI template must differ from EN for $messageId"
    }
    $env:VOLUMECONTROL_LANG = 'en'

    $installRoot = Get-InstallRoot
    New-Item -ItemType Directory -Path $installRoot | Out-Null
    $env:APPDATA = Join-Path $testRoot 'Roaming'
    $configDirectory = Join-Path $env:APPDATA 'volume-control'
    New-Item -ItemType Directory -Path $configDirectory | Out-Null
    $configPath = Join-Path $configDirectory 'config.ini'
    [System.IO.File]::WriteAllText($configPath, "volume=50`n", [System.Text.UTF8Encoding]::new($false))
    $installerName = 'VolumeControl_1.2.3_x64-setup.exe'
    $installerPath = Join-Path $testRoot $installerName
    Copy-Item (Join-Path $env:WINDIR 'System32\cmd.exe') $installerPath
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $installerBytes = [System.IO.File]::ReadAllBytes($installerPath)
        try {
            $installerSha256 = ([System.BitConverter]::ToString($sha256.ComputeHash($installerBytes))).Replace('-', '').ToLowerInvariant()
        } finally {
            [Array]::Clear($installerBytes, 0, $installerBytes.Length)
        }
    } finally {
        $sha256.Dispose()
    }

    $manifestPath = Join-Path $testRoot 'SHA256SUMS.txt'
    [System.IO.File]::WriteAllText($manifestPath, "$installerSha256  $installerName`n", [System.Text.UTF8Encoding]::new($false))
    $duplicateManifest = Join-Path $testRoot 'SHA256SUMS-duplicate.txt'
    $malformedManifest = Join-Path $testRoot 'SHA256SUMS-malformed.txt'
    [System.IO.File]::WriteAllText($malformedManifest, "$installerSha256 $installerName`n", [System.Text.UTF8Encoding]::new($false))
    [System.IO.File]::WriteAllText($duplicateManifest, "$installerSha256  $installerName`n$installerSha256  $installerName`n", [System.Text.UTF8Encoding]::new($false))

    Assert-Throws { Resolve-VolumeControlRelease -Version '1.2.3' } 'must start with v'

    $missingInstallerRelease = [pscustomobject]@{
        tag_name = 'v1.2.3'
        draft = $false
        prerelease = $false
        assets = @([pscustomobject]@{
            name = 'SHA256SUMS.txt'
            browser_download_url = 'https://example.invalid/SHA256SUMS.txt'
        })
    }
    Assert-Throws { Get-ReleaseInstaller -Release $missingInstallerRelease } 'missing installer asset'
    $validRelease = [pscustomobject]@{
        tag_name = 'v1.2.3'
        draft = $false
        prerelease = $false
        assets = @(
            [pscustomobject]@{
                name = $installerName
                browser_download_url = 'https://example.invalid/VolumeControl_1.2.3_x64-setup.exe'
            },
            [pscustomobject]@{
                name = 'SHA256SUMS.txt'
                browser_download_url = 'https://example.invalid/SHA256SUMS.txt'
            }
        )
    }
    $script:releaseRequestUri = $null
    $script:releaseResponse = $validRelease
    function global:Invoke-RestMethod {
        [CmdletBinding()]
        param(
            [string] $Uri,
            [hashtable] $Headers
        )

        $script:releaseRequestUri = $Uri
        return $script:releaseResponse
    }
    [void](Resolve-VolumeControlRelease)
    Assert-Equal $script:releaseRequestUri 'https://api.github.com/repos/dt418/volume-control/releases/latest' 'omitted version selects latest endpoint'
    [void](Resolve-VolumeControlRelease -Version 'v1.2.3')
    Assert-Equal $script:releaseRequestUri 'https://api.github.com/repos/dt418/volume-control/releases/tags/v1.2.3' 'exact version selects tag endpoint'
    $script:releaseResponse = [pscustomobject]@{
        tag_name = 'v1.2.3'
        draft = $true
        prerelease = $false
        assets = @()
    }
    Assert-Throws { Resolve-VolumeControlRelease } 'must not be draft or prerelease'
    Remove-Item function:global:Invoke-RestMethod -Force

    $releaseInstaller = Get-ReleaseInstaller -Release $validRelease
    $duplicateInstallerRelease = [pscustomobject]@{
        tag_name = $validRelease.tag_name
        draft = $false
        prerelease = $false
        assets = @($validRelease.assets) + @($validRelease.assets[0])
    }
    $duplicateManifestRelease = [pscustomobject]@{
        tag_name = $validRelease.tag_name
        draft = $false
        prerelease = $false
        assets = @($validRelease.assets) + @($validRelease.assets[1])
    }
    Assert-Throws { Get-ReleaseInstaller -Release $duplicateInstallerRelease } 'exactly one installer asset'
    Assert-Throws { Get-ReleaseInstaller -Release $duplicateManifestRelease } 'exactly one checksum manifest asset'
    Assert-Throws { Read-ExpectedSha256 -ManifestPath $malformedManifest -AssetName $installerName } 'not canonical'
    Assert-Equal $releaseInstaller.Installer.name $installerName 'release selects exact installer name'
    Assert-Equal (Read-ExpectedSha256 -ManifestPath $manifestPath -AssetName $installerName) $installerSha256 'canonical manifest returns lowercase checksum'
    Assert-Throws { Read-ExpectedSha256 -ManifestPath $duplicateManifest -AssetName $installerName } 'exactly one checksum entry'
    Assert-False (Test-FileSha256 -Path $installerPath -ExpectedSha256 ('0' * 64)) 'checksum mismatch must refuse installer'
    $actualStartInstaller = (Get-Item function:Start-Installer).ScriptBlock
    $script:expectedDownloadedSha256 = ('0' * 64)
    $script:installerLaunches = 0
    $script:lastDownloadRoot = $null
    function global:Invoke-WebRequest {
        [CmdletBinding()]
        param(
            [string] $Uri,
            [string] $OutFile,
            [switch] $UseBasicParsing
        )

        $script:lastDownloadRoot = Split-Path $OutFile -Parent
        if ($OutFile.EndsWith('SHA256SUMS.txt', [System.StringComparison]::Ordinal)) {
            [System.IO.File]::WriteAllText($OutFile, "$script:expectedDownloadedSha256  $installerName`n", [System.Text.UTF8Encoding]::new($false))
        } else {
            Copy-Item $installerPath $OutFile
        }
    }
    function Start-Installer {
        param([string] $InstallerPath, [switch] $Silent, [string] $InstallRoot)

        $script:installerLaunches += 1
    }

    Assert-Throws { Invoke-VerifiedInstaller -Release $validRelease -InstallRoot $installRoot } 'SHA-256 does not match'
    Assert-Equal $script:installerLaunches 0 'checksum mismatch must not launch installer'
    Assert-True (Test-Path $script:lastDownloadRoot) 'failed verification must retain downloaded evidence'
    Remove-Item -LiteralPath $script:lastDownloadRoot -Recurse -Force

    $script:expectedDownloadedSha256 = $installerSha256
    Invoke-VerifiedInstaller -Release $validRelease -InstallRoot $installRoot
    Assert-Equal $script:installerLaunches 1 'verified installer launches once'
    Assert-False (Test-Path $script:lastDownloadRoot) 'successful child completion removes downloads'

    Set-Item function:Start-Installer -Value $actualStartInstaller
    Remove-Item function:global:Invoke-WebRequest -Force


    $outsideRoot = Join-Path $testRoot 'outside'
    Assert-Throws { Start-Installer -InstallerPath $installerPath -Silent -InstallRoot $outsideRoot } 'install root must equal'

    $script:capturedInstallerArguments = $null
    $script:forcedProcessExitCode = $null
    function global:Start-Process {
        param(
            [string] $FilePath,
            [string[]] $ArgumentList = @(),
            [switch] $Wait,
            [switch] $PassThru
        )

        $script:capturedInstallerArguments = @($ArgumentList)
        $script:capturedInstallerHasArgumentList = $PSBoundParameters.ContainsKey('ArgumentList')
        $script:capturedInstallerWaitsForDescendants = $PSBoundParameters.ContainsKey('Wait')
        $script:capturedInstallerPassThru = $PSBoundParameters.ContainsKey('PassThru')
        if ($null -ne $script:forcedProcessExitCode) {
            $failedProcess = [pscustomobject]@{ ExitCode = $script:forcedProcessExitCode }
            $failedProcess | Add-Member -MemberType ScriptMethod -Name WaitForExit -Value { }
            return $failedProcess
        }
        $startInfo = New-Object System.Diagnostics.ProcessStartInfo
        $startInfo.FileName = $FilePath
        $startInfo.Arguments = [string]::Join(' ', $ArgumentList)
        $startInfo.UseShellExecute = $false
        $startInfo.RedirectStandardInput = $true
        $child = New-Object System.Diagnostics.Process
        $child.StartInfo = $startInfo
        [void]$child.Start()
        $child.StandardInput.Close()
        $child.WaitForExit()
        return $child
    }

    Start-Installer -InstallerPath $installerPath -InstallRoot $installRoot
    Assert-Equal $script:capturedInstallerArguments.Count 0 'GUI installer must receive no arguments'
    Assert-False $script:capturedInstallerHasArgumentList 'GUI installer must omit ArgumentList'
    Assert-False $script:capturedInstallerWaitsForDescendants 'wait must not span descendants: NSIS launches VolumeControl'
    Assert-True $script:capturedInstallerPassThru 'wait tracks the started process explicitly'

    Start-Installer -InstallerPath $installerPath -Silent -InstallRoot $installRoot
    Assert-Equal $script:capturedInstallerArguments.Count 2 'silent installer must receive two arguments'
    Assert-Equal $script:capturedInstallerArguments[0] '/S' 'silent installer first argument'
    Assert-Equal $script:capturedInstallerArguments[1] "/D=$installRoot" 'silent installer install-root argument'
    Assert-True $script:capturedInstallerHasArgumentList 'silent installer must pass ArgumentList'
    $script:forcedProcessExitCode = 17
    Assert-Throws { Start-Installer -InstallerPath $installerPath -Silent -InstallRoot $installRoot } 'process failed with exit code 17'
    $script:forcedProcessExitCode = $null

    $uninstallerPath = Join-Path $installRoot 'uninstall.exe'
    $outsideUninstaller = Join-Path $testRoot 'outside-uninstall.exe'
    Copy-Item $installerPath $outsideUninstaller
    Assert-Throws { Start-Uninstaller -UninstallerPath $outsideUninstaller -Silent -InstallRoot $installRoot } 'uninstaller must equal'
    Copy-Item $installerPath $uninstallerPath
    Start-Uninstaller -UninstallerPath $uninstallerPath -Silent -InstallRoot $installRoot
    Assert-Equal $script:capturedInstallerArguments.Count 1 'silent uninstaller must receive one argument'
    Assert-Equal $script:capturedInstallerArguments[0] '/S' 'silent uninstaller argument'
    $uninstallOutput = @(Invoke-VolumeControlAction -Action uninstall -Silent -InstallRoot $installRoot 6>&1)
    Assert-Equal ($uninstallOutput -join "`n") "removed VolumeControl from $installRoot" 'uninstall reports the result'
    Assert-True (Test-Path -LiteralPath $configPath -PathType Leaf) 'uninstall must preserve %APPDATA%\volume-control'
    Remove-Item -LiteralPath $uninstallerPath -Force -ErrorAction SilentlyContinue

    # Second uninstall against clean root must be successful no-op
    $secondUninstallOutput = @(Invoke-VolumeControlAction -Action uninstall -Silent -InstallRoot $installRoot 6>&1)
    Assert-Equal ($secondUninstallOutput -join "`n") "nothing installed at $installRoot" 'second uninstall reports nothing installed'

    Remove-Item function:global:Start-Process -Force
    $actualInvokeVerifiedInstaller = (Get-Item function:Invoke-VerifiedInstaller).ScriptBlock
    $script:currentVersionLaunches = 0
    function Invoke-VerifiedInstaller {
        param($Release, [string] $InstallRoot, [switch] $Silent)

        $script:currentVersionLaunches += 1
    }

    $installOutput = @(Invoke-VolumeControlAction -Action install -Release $validRelease -InstallRoot $installRoot 6>&1)
    Assert-Equal ($installOutput -join "`n") "installed VolumeControl 1.2.3 into $installRoot" 'install reports the result'
    Assert-Equal $script:currentVersionLaunches 1 'install launches the verified installer once'
    $installedExecutable = Join-Path $installRoot 'VolumeControl.exe'
    Copy-Item $installerPath $installedExecutable
    $script:installedFileVersion = '1.0.0'
    function global:Get-Item {
        [CmdletBinding()]
        param([string] $LiteralPath)

        if ($LiteralPath -ceq $installedExecutable) {
            return [pscustomobject]@{
                VersionInfo = [pscustomobject]@{ FileVersion = $script:installedFileVersion }
            }
        }
        return Microsoft.PowerShell.Management\Get-Item @PSBoundParameters
    }
    function global:Get-Process {
        [CmdletBinding()]
        param([string] $Name)

        if ($Name -ceq 'VolumeControl') {
            return $null
        }
        return Microsoft.PowerShell.Management\Get-Process @PSBoundParameters
    }

    $updateOutput = @(Invoke-VolumeControlAction -Action update -Release $validRelease -InstallRoot $installRoot 6>&1)
    Assert-Equal ($updateOutput -join "`n") "updated VolumeControl 1.2.3 at $installRoot" 'update reports the result'
    Assert-Equal $script:currentVersionLaunches 2 'stale install launches the verified installer once more'

    Remove-Item function:global:Get-Process -Force
    $script:installedFileVersion = '1.2.3'
    Copy-Item $installerPath (Join-Path $installRoot 'uninstall.exe') -Force
    Write-InstallReceipt -InstallRoot $installRoot -Tag 'v1.2.3' -ExecutableSha256 $installerSha256
    $currentOutput = @(Invoke-VolumeControlAction -Action update -Release $validRelease -InstallRoot $installRoot 6>&1)
    Assert-Equal ($currentOutput -join "`n") 'VolumeControl 1.2.3 is already installed; nothing to update' 'current release reports the no-op'
    Assert-Equal $script:currentVersionLaunches 2 'current release update must not launch installer'

    # Test newer installed version reports no downgrade
    $script:installedFileVersion = '2.0.0'
    $downgradeOutput = @(Invoke-VolumeControlAction -Action install -Release $validRelease -InstallRoot $installRoot 6>&1)
    Assert-True (($downgradeOutput -join "`n") -like '*skipping downgrade*') 'installed newer version skips downgrade'
    Assert-Equal $script:currentVersionLaunches 2 'downgrade must not launch installer'

    Remove-Item function:global:Get-Item -Force
    Set-Item function:Invoke-VerifiedInstaller -Value $actualInvokeVerifiedInstaller

    $installedExecutable = Join-Path $installRoot 'VolumeControl.exe'
    Copy-Item $installerPath $installedExecutable -Force
    $newRelease = [pscustomobject]@{
        tag_name = 'v9.9.9'
        draft = $false
        prerelease = $false
        assets = @()
    }
    $script:StopTimeoutSeconds = 0
    function global:Get-Process {
        [CmdletBinding()]
        param([string] $Name)

        if ($Name -ceq 'VolumeControl') {
            return [pscustomobject]@{ Name = 'VolumeControl'; Id = 4242; Path = $installedExecutable }
        }
        return Microsoft.PowerShell.Management\Get-Process @PSBoundParameters
    }
    function global:Stop-Process {
        throw 'bootstrap must not force-terminate VolumeControl'
    }

    Assert-Throws { Invoke-VolumeControlAction -Action update -Release $newRelease -InstallRoot $installRoot } 'close VolumeControl'
    $script:StopTimeoutSeconds = $null
    Remove-Item function:global:Get-Process -Force
    Remove-Item function:global:Stop-Process -Force
    # Prepared apply test
    $prepDir = Join-Path $testRoot 'prepared-test'
    New-Item -ItemType Directory -Path $prepDir | Out-Null
    Copy-Item $installerPath (Join-Path $prepDir $installerName)
    Copy-Item $manifestPath (Join-Path $prepDir 'SHA256SUMS.txt')
    Invoke-VolumeControlAction -Action update -PreparedDir $prepDir -InstallRoot $installRoot
    Assert-True (Test-Path (Join-Path $prepDir 'handoff.ready')) 'prepared apply writes handoff.ready'
    Assert-True (Test-Path (Join-Path $prepDir 'result.exit-code')) 'prepared apply writes result.exit-code'
    Assert-Equal ([System.IO.File]::ReadAllText((Join-Path $prepDir 'result.exit-code')).Trim()) '0' 'result.exit-code is 0'
    # --- Remote-download safety: `irm` on Windows PowerShell 5.1 decodes a
    # text/plain response as cp1252, so the published asset must be pure ASCII
    # and BOM-less; VI text travels as [char] escapes that survive that decode.
    $bootstrapSourcePath = Join-Path $PSScriptRoot 'Install-VolumeControl.ps1'
    $bootstrapBytes = [System.IO.File]::ReadAllBytes($bootstrapSourcePath)
    $nonAsciiCount = 0
    foreach ($byte in $bootstrapBytes) {
        if ($byte -gt 127) { $nonAsciiCount++ }
    }
    Assert-Equal $nonAsciiCount 0 'bootstrap source stays pure ASCII for cp1252-safe downloads'
    Assert-False ($bootstrapBytes.Length -ge 3 -and $bootstrapBytes[0] -eq 0xEF -and $bootstrapBytes[1] -eq 0xBB -and $bootstrapBytes[2] -eq 0xBF) 'bootstrap source carries no UTF-8 BOM'

    $cp1252 = [System.Text.Encoding]::GetEncoding(1252)
    $decodedBootstrap = $cp1252.GetString($bootstrapBytes)
    $decodedBlock = [scriptblock]::Create($decodedBootstrap)
    Assert-True ($null -ne $decodedBlock) 'cp1252-decoded bootstrap parses as a scriptblock'

    $viFromDecoded = & {
        . ([scriptblock]::Create($decodedBootstrap))
        $env:VOLUMECONTROL_LANG = 'vi'
        Get-VolumeControlMessage 'install-root.must-equal' 'C:\root'
    }
    Assert-Equal $viFromDecoded "th$([char]0x01B0) m$([char]0x1EE5)c c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t ph$([char]0x1EA3)i b$([char]0x1EB1)ng C:\root" 'cp1252-decoded bootstrap renders VI text correctly'

    # --- VI scenario set: $env:VOLUMECONTROL_LANG override ---
    $previousVolumeControlLang = $env:VOLUMECONTROL_LANG
    $previousUiCulture = [System.Globalization.CultureInfo]::CurrentUICulture
    try {
        Remove-Item Env:\VOLUMECONTROL_LANG -ErrorAction SilentlyContinue
        [System.Globalization.CultureInfo]::CurrentUICulture = [System.Globalization.CultureInfo]::GetCultureInfo('en-US')
        Assert-Equal (Get-VolumeControlLanguage) 'en' 'EN UI culture selects EN without an override'
        [System.Globalization.CultureInfo]::CurrentUICulture = [System.Globalization.CultureInfo]::GetCultureInfo('vi-VN')
        Assert-Equal (Get-VolumeControlLanguage) 'vi' 'VI UI culture selects VI without an override'
        $env:VOLUMECONTROL_LANG = 'EN'
        Assert-Equal (Get-VolumeControlLanguage) 'en' 'EN override wins over the VI UI culture'
        $env:VOLUMECONTROL_LANG = 'Vi'
        Assert-Equal (Get-VolumeControlLanguage) 'vi' 'language override is case-insensitive'
        $env:VOLUMECONTROL_LANG = 'fr'
        Assert-Equal (Get-VolumeControlLanguage) 'en' 'unknown language override falls back to EN'

        $env:VOLUMECONTROL_LANG = 'vi'
        Assert-Equal (Get-VolumeControlMessage 'install-root.must-equal' 'C:\root') "th$([char]0x01B0) m$([char]0x1EE5)c c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t ph$([char]0x1EA3)i b$([char]0x1EB1)ng C:\root" 'VI template stays byte-identical'
        Assert-Throws { Resolve-VolumeControlRelease -Version '1.2.3' } "phi$([char]0x00EA)n b$([char]0x1EA3)n ph$([char]0x1EA3)i b$([char]0x1EAF)t $([char]0x0111)$([char]0x1EA7)u b$([char]0x1EB1)ng v"
        Assert-Throws { Assert-StandardInstallRoot -InstallRoot $outsideRoot } "th$([char]0x01B0) m$([char]0x1EE5)c c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t ph$([char]0x1EA3)i b$([char]0x1EB1)ng"
        Assert-Throws { Read-ExpectedSha256 -ManifestPath $malformedManifest -AssetName $installerName } "kh$([char]0x00F4)ng $([char]0x0111)$([char]0x00FA)ng $([char]0x0111)$([char]0x1ECB)nh d$([char]0x1EA1)ng chu$([char]0x1EA9)n"

        $viFailureMessage = $null
        $script:StopTimeoutSeconds = 0
        function global:Get-Process {
            [CmdletBinding()]
            param([string] $Name)
            if ($Name -ceq 'VolumeControl') {
                return [pscustomobject]@{ Name = 'VolumeControl'; Id = 4242; Path = $installedExecutable }
            }
            return Microsoft.PowerShell.Management\Get-Process @PSBoundParameters
        }
        try {
            Invoke-VolumeControlAction -Action update -Release $newRelease -InstallRoot $installRoot
        } catch {
            $viFailureMessage = $_.Exception.Message
        } finally {
            $script:StopTimeoutSeconds = $null
            Remove-Item function:global:Get-Process -Force
        }
        Assert-True ($viFailureMessage -clike "*h$([char]0x00E3)y $([char]0x0111)$([char]0x00F3)ng VolumeControl*") 'VI message for the running-process failure path'
        Assert-False ($viFailureMessage -clike '*update.refused-running-process*') 'message IDs never reach the user'

        Start-Installer -InstallerPath $installerPath -InstallRoot $installRoot
        Assert-Equal (Read-ExpectedSha256 -ManifestPath $manifestPath -AssetName $installerName) $installerSha256 'successful path stays intact under the VI override'
    } finally {
        [System.Globalization.CultureInfo]::CurrentUICulture = $previousUiCulture
        if ([string]::IsNullOrEmpty($previousVolumeControlLang)) {
            Remove-Item Env:\VOLUMECONTROL_LANG -ErrorAction SilentlyContinue
        } else {
            $env:VOLUMECONTROL_LANG = $previousVolumeControlLang
        }
    }
} finally {
    foreach ($commandName in 'Start-Process', 'Get-Item', 'Get-Process', 'Stop-Process') {
        $functionPath = "function:global:$commandName"
        if (Test-Path $functionPath) {
            Remove-Item $functionPath -Force
        }
    }
    foreach ($originalCommand in @($originalStartProcess, $originalGetItem, $originalGetProcess, $originalStopProcess)) {
        if ($null -ne $originalCommand) {
            Set-Item -Path "function:global:$($originalCommand.Name)" -Value $originalCommand.ScriptBlock
        }
    }
    $env:LOCALAPPDATA = $originalLocalAppData
    $env:APPDATA = $originalAppData
    if ([string]::IsNullOrEmpty($originalVolumeControlLang)) {
        Remove-Item Env:\VOLUMECONTROL_LANG -ErrorAction SilentlyContinue
    } else {
        $env:VOLUMECONTROL_LANG = $originalVolumeControlLang
    }
    Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host 'PASS: Install-VolumeControl bootstrap behavioral tests'
