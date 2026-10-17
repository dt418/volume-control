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

# User-facing message table. Keys are stable message IDs and stay internal:
# only the rendered template is ever printed. The English templates are the
# canonical text; `{0}` placeholders are filled by Get-VolumeControlMessage.
$script:VolumeControlMessages = @{
    'install-root.localappdata-required' = @{
        en = 'LOCALAPPDATA is required to determine the install root'
        vi = 'Cần biến môi trường LOCALAPPDATA để xác định thư mục cài đặt'
    }
    'install-root.must-equal' = @{
        en = 'install root must equal {0}'
        vi = 'thư mục cài đặt phải bằng {0}'
    }
    'release.draft-or-prerelease' = @{
        en = 'release must not be draft or prerelease'
        vi = 'bản phát hành không được ở trạng thái draft hoặc prerelease'
    }
    'release.tag-format' = @{
        en = 'release tag must match vMAJOR.MINOR.PATCH'
        vi = 'thẻ phát hành phải khớp định dạng vMAJOR.MINOR.PATCH'
    }
    'version.format' = @{
        en = 'version must start with v and match vMAJOR.MINOR.PATCH'
        vi = 'phiên bản phải bắt đầu bằng v và khớp định dạng vMAJOR.MINOR.PATCH'
    }
    'asset.installer-missing' = @{
        en = 'missing installer asset: {0}'
        vi = 'thiếu tệp cài đặt: {0}'
    }
    'asset.installer-duplicate' = @{
        en = 'expected exactly one installer asset: {0}'
        vi = 'phải có đúng một tệp cài đặt: {0}'
    }
    'asset.manifest-missing' = @{
        en = 'missing checksum manifest asset: {0}'
        vi = 'thiếu tệp manifest checksum: {0}'
    }
    'asset.manifest-duplicate' = @{
        en = 'expected exactly one checksum manifest asset: {0}'
        vi = 'phải có đúng một tệp manifest checksum: {0}'
    }
    'asset.download-url-missing' = @{
        en = 'release asset download URL is missing'
        vi = 'thiếu URL tải xuống của tệp phát hành'
    }
    'manifest.asset-basename' = @{
        en = 'asset name must be a basename'
        vi = 'tên tệp phải là tên cơ sở, không chứa đường dẫn'
    }
    'manifest.not-found' = @{
        en = 'checksum manifest does not exist: {0}'
        vi = 'không tìm thấy tệp manifest checksum: {0}'
    }
    'manifest.entry-not-canonical' = @{
        en = 'checksum manifest entry is not canonical'
        vi = 'mục trong manifest checksum không đúng định dạng chuẩn'
    }
    'manifest.entry-count' = @{
        en = 'checksum manifest must contain exactly one checksum entry for {0}'
        vi = 'manifest checksum phải chứa đúng một mục checksum cho {0}'
    }
    'process.exit-code' = @{
        en = 'installer process failed with exit code {0}'
        vi = 'tiến trình cài đặt thất bại với mã thoát {0}'
    }
    'installer.not-found' = @{
        en = 'installer does not exist: {0}'
        vi = 'không tìm thấy tệp cài đặt: {0}'
    }
    'uninstaller.must-equal' = @{
        en = 'uninstaller must equal {0}'
        vi = 'tệp gỡ cài đặt phải bằng {0}'
    }
    'uninstaller.not-found' = @{
        en = 'uninstaller does not exist: {0}'
        vi = 'không tìm thấy tệp gỡ cài đặt: {0}'
    }
    'asset.download-failed' = @{
        en = 'asset download did not create {0}'
        vi = 'quá trình tải tệp không tạo ra {0}'
    }
    'installer.sha256-mismatch' = @{
        en = 'installer SHA-256 does not match {0}'
        vi = 'SHA-256 của tệp cài đặt không khớp với {0}'
    }
    'install.refused-executable-exists' = @{
        en = 'install refused because standard executable already exists: {0}'
        vi = 'từ chối cài đặt vì tệp thực thi chuẩn đã tồn tại: {0}'
    }
    'update.refused-executable-missing' = @{
        en = 'update refused because standard executable is missing: {0}'
        vi = 'từ chối cập nhật vì thiếu tệp thực thi chuẩn: {0}'
    }
    'update.refused-running-process' = @{
        en = 'close VolumeControl before updating; bootstrap will not force-terminate it'
        vi = 'hãy đóng VolumeControl trước khi cập nhật; bootstrap sẽ không buộc kết thúc tiến trình'
    }
}

function Get-VolumeControlLanguage {
    [CmdletBinding()]
    param()

    $override = $env:VOLUMECONTROL_LANG
    if (-not [string]::IsNullOrWhiteSpace($override)) {
        if ($override.Trim().ToLowerInvariant() -ceq 'vi') {
            return 'vi'
        }

        return 'en'
    }

    if ([System.Globalization.CultureInfo]::CurrentUICulture.TwoLetterISOLanguageName -ceq 'vi') {
        return 'vi'
    }

    return 'en'
}

function Get-VolumeControlMessage {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0)][string] $Id,
        [Parameter(Position = 1, ValueFromRemainingArguments = $true)][object[]] $ArgumentList = @()
    )

    $entry = $script:VolumeControlMessages[$Id]
    if ($null -eq $entry) {
        throw "unknown installer message id: $Id"
    }

    $template = $entry[(Get-VolumeControlLanguage)]
    if (@($ArgumentList).Count -eq 0) {
        return $template
    }

    return ($template -f $ArgumentList)
}

function Get-InstallRoot {
    [CmdletBinding()]
    param()

    if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        throw (Get-VolumeControlMessage 'install-root.localappdata-required')
    }

    return Join-Path $env:LOCALAPPDATA 'VolumeControl'
}

function Assert-StandardInstallRoot {
    param([Parameter(Mandatory = $true)][string] $InstallRoot)

    $expectedRoot = [System.IO.Path]::GetFullPath((Get-InstallRoot))
    $actualRoot = [System.IO.Path]::GetFullPath($InstallRoot)
    if (-not [string]::Equals($actualRoot, $expectedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw (Get-VolumeControlMessage 'install-root.must-equal' $expectedRoot)
    }

    return $expectedRoot
}

function Assert-EligibleRelease {
    param([Parameter(Mandatory = $true)] $Release)

    if ($Release.draft -ne $false -or $Release.prerelease -ne $false) {
        throw (Get-VolumeControlMessage 'release.draft-or-prerelease')
    }
    if ($Release.tag_name -isnot [string] -or $Release.tag_name -notmatch '^v\d+\.\d+\.\d+$') {
        throw (Get-VolumeControlMessage 'release.tag-format')
    }

    return $Release
}

function Resolve-VolumeControlRelease {
    [CmdletBinding()]
    param([string] $Version)

    if (-not [string]::IsNullOrEmpty($Version) -and $Version -notmatch '^v\d+\.\d+\.\d+$') {
        throw (Get-VolumeControlMessage 'version.format')
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
        throw (Get-VolumeControlMessage 'asset.installer-missing' $installerName)
    }
    if ($installers.Count -ne 1) {
        throw (Get-VolumeControlMessage 'asset.installer-duplicate' $installerName)
    }
    if ($manifests.Count -eq 0) {
        throw (Get-VolumeControlMessage 'asset.manifest-missing' $script:ChecksumManifestName)
    }
    if ($manifests.Count -ne 1) {
        throw (Get-VolumeControlMessage 'asset.manifest-duplicate' $script:ChecksumManifestName)
    }
    if ([string]::IsNullOrWhiteSpace($installers[0].browser_download_url) -or [string]::IsNullOrWhiteSpace($manifests[0].browser_download_url)) {
        throw (Get-VolumeControlMessage 'asset.download-url-missing')
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
        throw (Get-VolumeControlMessage 'manifest.asset-basename')
    }
    if (-not [System.IO.File]::Exists($ManifestPath)) {
        throw (Get-VolumeControlMessage 'manifest.not-found' $ManifestPath)
    }

    $expectedChecksums = New-Object System.Collections.Generic.List[string]
    foreach ($line in [System.IO.File]::ReadAllLines($ManifestPath)) {
        if ($line -notmatch '^([0-9A-Fa-f]{64})  ([^\\/]+)$') {
            throw (Get-VolumeControlMessage 'manifest.entry-not-canonical')
        }
        if ($Matches[2] -ceq $AssetName) {
            $expectedChecksums.Add($Matches[1].ToLowerInvariant())
        }
    }

    if ($expectedChecksums.Count -ne 1) {
        throw (Get-VolumeControlMessage 'manifest.entry-count' $AssetName)
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
        throw (Get-VolumeControlMessage 'process.exit-code' $process.ExitCode)
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
        throw (Get-VolumeControlMessage 'installer.not-found' $InstallerPath)
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
        throw (Get-VolumeControlMessage 'uninstaller.must-equal' $expectedUninstaller)
    }
    if (-not [System.IO.File]::Exists($actualUninstaller)) {
        throw (Get-VolumeControlMessage 'uninstaller.not-found' $actualUninstaller)
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
        throw (Get-VolumeControlMessage 'asset.download-failed' $DestinationPath)
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
            throw (Get-VolumeControlMessage 'installer.sha256-mismatch' $script:ChecksumManifestName)
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
            throw (Get-VolumeControlMessage 'install.refused-executable-exists' $installedExecutable)
        }
        Invoke-VerifiedInstaller -Release $Release -InstallRoot $standardRoot -Silent:$Silent
        return
    }

    if (-not [System.IO.File]::Exists($installedExecutable)) {
        throw (Get-VolumeControlMessage 'update.refused-executable-missing' $installedExecutable)
    }

    $selectedVersion = $Release.tag_name.Substring(1)
    $installedVersion = (Get-Item -LiteralPath $installedExecutable).VersionInfo.FileVersion
    if ($installedVersion -ceq $selectedVersion) {
        return
    }

    if ($null -ne (Get-Process -Name 'VolumeControl' -ErrorAction SilentlyContinue)) {
        throw (Get-VolumeControlMessage 'update.refused-running-process')
    }

    Invoke-VerifiedInstaller -Release $Release -InstallRoot $standardRoot -Silent:$Silent
}

if ($MyInvocation.InvocationName -ne '.') {
    Invoke-VolumeControlAction -Action $Action -Version $Version -Silent:$Silent
}
