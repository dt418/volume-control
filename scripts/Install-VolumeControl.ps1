#!/usr/bin/env powershell
[CmdletBinding()]
param(
    [ValidateSet('install', 'update', 'uninstall')]
    [string] $Action = 'install',

    [ValidatePattern('^v[0-9]+\.[0-9]+\.[0-9]+$')]
    [string] $Version,

    [switch] $Silent
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:VolumeControlRepository = 'dt418/volume-control'
$script:VolumeControlReleaseApi = "https://api.github.com/repos/$script:VolumeControlRepository/releases"
$script:ChecksumManifestName = 'SHA256SUMS.txt'

function Get-InstallRoot {
    [CmdletBinding()]
    param()

    if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        throw 'LOCALAPPDATA is required to determine the install root'
    }

    return Join-Path $env:LOCALAPPDATA 'VolumeControl'
}

function Assert-StandardInstallRoot {
    param([Parameter(Mandatory = $true)][string] $InstallRoot)

    $expectedRoot = [System.IO.Path]::GetFullPath((Get-InstallRoot))
    $actualRoot = [System.IO.Path]::GetFullPath($InstallRoot)
    if (-not [string]::Equals($actualRoot, $expectedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "install root must equal $expectedRoot"
    }

    return $expectedRoot
}

function Assert-EligibleRelease {
    param([Parameter(Mandatory = $true)] $Release)

    if ($Release.draft -ne $false -or $Release.prerelease -ne $false) {
        throw 'release must not be draft or prerelease'
    }
    if ($Release.tag_name -isnot [string] -or $Release.tag_name -notmatch '^v\d+\.\d+\.\d+$') {
        throw 'release tag must match vMAJOR.MINOR.PATCH'
    }

    return $Release
}

function Resolve-VolumeControlRelease {
    [CmdletBinding()]
    param([string] $Version)

    if (-not [string]::IsNullOrEmpty($Version) -and $Version -notmatch '^v\d+\.\d+\.\d+$') {
        throw 'version must start with v and match vMAJOR.MINOR.PATCH'
    }

    $headers = @{ Accept = 'application/vnd.github+json' }
    if ([string]::IsNullOrEmpty($Version)) {
        $release = Invoke-RestMethod -Uri "$script:VolumeControlReleaseApi/latest" -Headers $headers -ErrorAction Stop
    } else {
        $release = Invoke-RestMethod -Uri "$script:VolumeControlReleaseApi/tags/$Version" -Headers $headers -ErrorAction Stop
    }

    return Assert-EligibleRelease -Release $release
}

function Get-ReleaseInstaller {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] $Release)

    $release = Assert-EligibleRelease -Release $Release
    $installerName = "VolumeControl_$($release.tag_name.Substring(1))_x64-setup.exe"
    $assets = @($release.assets)
    $installers = @($assets | Where-Object { $_.name -ceq $installerName })
    $manifests = @($assets | Where-Object { $_.name -ceq $script:ChecksumManifestName })

    if ($installers.Count -eq 0) {
        throw "missing installer asset: $installerName"
    }
    if ($installers.Count -ne 1) {
        throw "expected exactly one installer asset: $installerName"
    }
    if ($manifests.Count -eq 0) {
        throw "missing checksum manifest asset: $script:ChecksumManifestName"
    }
    if ($manifests.Count -ne 1) {
        throw "expected exactly one checksum manifest asset: $script:ChecksumManifestName"
    }
    if ([string]::IsNullOrWhiteSpace($installers[0].browser_download_url) -or [string]::IsNullOrWhiteSpace($manifests[0].browser_download_url)) {
        throw 'release asset download URL is missing'
    }

    return [pscustomobject]@{
        Version = $release.tag_name
        Installer = $installers[0]
        Manifest = $manifests[0]
    }
}

function Read-ExpectedSha256 {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string] $ManifestPath,
        [Parameter(Mandatory = $true)][string] $AssetName
    )

    if ([System.IO.Path]::GetFileName($AssetName) -cne $AssetName) {
        throw 'asset name must be a basename'
    }
    if (-not [System.IO.File]::Exists($ManifestPath)) {
        throw "checksum manifest does not exist: $ManifestPath"
    }

    $expectedChecksums = New-Object System.Collections.Generic.List[string]
    foreach ($line in [System.IO.File]::ReadAllLines($ManifestPath)) {
        if ($line -notmatch '^([0-9A-Fa-f]{64})  ([^\\/]+)$') {
            throw 'checksum manifest entry is not canonical'
        }
        if ($Matches[2] -ceq $AssetName) {
            $expectedChecksums.Add($Matches[1].ToLowerInvariant())
        }
    }

    if ($expectedChecksums.Count -ne 1) {
        throw "checksum manifest must contain exactly one checksum entry for $AssetName"
    }

    return $expectedChecksums[0]
}

function Test-FileSha256 {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)][string] $ExpectedSha256
    )

    if ($ExpectedSha256 -notmatch '^[0-9A-Fa-f]{64}$' -or -not [System.IO.File]::Exists($Path)) {
        return $false
    }

    $stream = [System.IO.File]::Open($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $actualSha256 = ([System.BitConverter]::ToString($sha256.ComputeHash($stream))).Replace('-', '').ToLowerInvariant()
    } finally {
        $sha256.Dispose()
        $stream.Dispose()
    }

    return $actualSha256 -ceq $ExpectedSha256.ToLowerInvariant()
}

function Wait-ForSuccessfulProcess {
    param(
        [Parameter(Mandatory = $true)][string] $FilePath,
        [string[]] $ArgumentList = @()
    )

    if ($ArgumentList.Count -eq 0) {
        $process = Start-Process -FilePath $FilePath -PassThru -Wait
    } else {
        $process = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -PassThru -Wait
    }
    if ($process.ExitCode -ne 0) {
        throw "installer process failed with exit code $($process.ExitCode)"
    }
}

function Start-Installer {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string] $InstallerPath,
        [switch] $Silent,
        [Parameter(Mandatory = $true)][string] $InstallRoot
    )

    $standardRoot = Assert-StandardInstallRoot -InstallRoot $InstallRoot
    if (-not [System.IO.File]::Exists($InstallerPath)) {
        throw "installer does not exist: $InstallerPath"
    }

    $arguments = @()
    if ($Silent) {
        $arguments = @('/S', "/D=$standardRoot")
    }
    Wait-ForSuccessfulProcess -FilePath $InstallerPath -ArgumentList $arguments
}

function Start-Uninstaller {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string] $UninstallerPath,
        [switch] $Silent,
        [Parameter(Mandatory = $true)][string] $InstallRoot
    )

    $standardRoot = Assert-StandardInstallRoot -InstallRoot $InstallRoot
    $expectedUninstaller = Join-Path $standardRoot 'uninstall.exe'
    $actualUninstaller = [System.IO.Path]::GetFullPath($UninstallerPath)
    if (-not [string]::Equals($actualUninstaller, $expectedUninstaller, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "uninstaller must equal $expectedUninstaller"
    }
    if (-not [System.IO.File]::Exists($actualUninstaller)) {
        throw "uninstaller does not exist: $actualUninstaller"
    }

    $arguments = @()
    if ($Silent) {
        $arguments = @('/S')
    }
    Wait-ForSuccessfulProcess -FilePath $actualUninstaller -ArgumentList $arguments
}

function Save-ReleaseAsset {
    param(
        [Parameter(Mandatory = $true)] $Asset,
        [Parameter(Mandatory = $true)][string] $DestinationPath
    )

    Invoke-WebRequest -Uri $Asset.browser_download_url -OutFile $DestinationPath -UseBasicParsing -ErrorAction Stop
    if (-not [System.IO.File]::Exists($DestinationPath)) {
        throw "asset download did not create $DestinationPath"
    }
}

function Invoke-VerifiedInstaller {
    param(
        [Parameter(Mandatory = $true)] $Release,
        [Parameter(Mandatory = $true)][string] $InstallRoot,
        [switch] $Silent
    )

    $releaseInstaller = Get-ReleaseInstaller -Release $Release
    $downloadRoot = Join-Path $env:TEMP ("VolumeControl-release-" + [guid]::NewGuid().ToString('N'))
    $downloadedInstaller = Join-Path $downloadRoot $releaseInstaller.Installer.name
    $downloadedManifest = Join-Path $downloadRoot $releaseInstaller.Manifest.name
    $completed = $false

    New-Item -ItemType Directory -Path $downloadRoot -ErrorAction Stop | Out-Null
    try {
        Save-ReleaseAsset -Asset $releaseInstaller.Installer -DestinationPath $downloadedInstaller
        Save-ReleaseAsset -Asset $releaseInstaller.Manifest -DestinationPath $downloadedManifest
        $expectedSha256 = Read-ExpectedSha256 -ManifestPath $downloadedManifest -AssetName $releaseInstaller.Installer.name
        if (-not (Test-FileSha256 -Path $downloadedInstaller -ExpectedSha256 $expectedSha256)) {
            throw "installer SHA-256 does not match $script:ChecksumManifestName"
        }

        Start-Installer -InstallerPath $downloadedInstaller -Silent:$Silent -InstallRoot $InstallRoot
        $completed = $true
    } finally {
        if ($completed) {
            Remove-Item -LiteralPath $downloadRoot -Recurse -Force -ErrorAction Stop
        }
    }
}

function Invoke-VolumeControlAction {
    [CmdletBinding()]
    param(
        [ValidateSet('install', 'update', 'uninstall')]
        [string] $Action = 'install',
        [string] $Version,
        [switch] $Silent,
        $Release,
        [string] $InstallRoot = (Get-InstallRoot)
    )

    $standardRoot = Assert-StandardInstallRoot -InstallRoot $InstallRoot
    $installedExecutable = Join-Path $standardRoot 'VolumeControl.exe'
    $uninstaller = Join-Path $standardRoot 'uninstall.exe'

    if ($Action -ceq 'uninstall') {
        Start-Uninstaller -UninstallerPath $uninstaller -Silent:$Silent -InstallRoot $standardRoot
        return
    }

    if ($null -eq $Release) {
        $Release = Resolve-VolumeControlRelease -Version $Version
    } else {
        $Release = Assert-EligibleRelease -Release $Release
    }

    if ($Action -ceq 'install') {
        if ([System.IO.File]::Exists($installedExecutable)) {
            throw "install refused because standard executable already exists: $installedExecutable"
        }
        Invoke-VerifiedInstaller -Release $Release -InstallRoot $standardRoot -Silent:$Silent
        return
    }

    if (-not [System.IO.File]::Exists($installedExecutable)) {
        throw "update refused because standard executable is missing: $installedExecutable"
    }

    $selectedVersion = $Release.tag_name.Substring(1)
    $installedVersion = (Get-Item -LiteralPath $installedExecutable).VersionInfo.FileVersion
    if ($installedVersion -ceq $selectedVersion) {
        return
    }

    if ($null -ne (Get-Process -Name 'VolumeControl' -ErrorAction SilentlyContinue)) {
        throw 'close VolumeControl before updating; bootstrap will not force-terminate it'
    }

    Invoke-VerifiedInstaller -Release $Release -InstallRoot $standardRoot -Silent:$Silent
}

if ($MyInvocation.InvocationName -ne '.') {
    Invoke-VolumeControlAction -Action $Action -Version $Version -Silent:$Silent
}
