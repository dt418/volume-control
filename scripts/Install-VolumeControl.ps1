#!/usr/bin/env powershell
[CmdletBinding()]
param(
    [ValidateSet('install', 'update', 'uninstall')]
    [string] $Action = 'install',

    [ValidatePattern('^v[0-9]+\.[0-9]+\.[0-9]+$')]
    [string] $Version,

    [switch] $Silent,

    [string] $PreparedDirectory,

    [int] $ParentProcessId = 0,

    [switch] $Restart,

    [switch] $StopOnly,

    [string] $ReleaseBaseUrl,

    [switch] $AllowTestSource
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
        vi = "C$([char]0x1EA7)n bi$([char]0x1EBF)n m$([char]0x00F4)i tr$([char]0x01B0)$([char]0x1EDD)ng LOCALAPPDATA $([char]0x0111)$([char]0x1EC3) x$([char]0x00E1)c $([char]0x0111)$([char]0x1ECB)nh th$([char]0x01B0) m$([char]0x1EE5)c c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t"
    }
    'install-root.must-equal' = @{
        en = 'install root must equal {0}'
        vi = "th$([char]0x01B0) m$([char]0x1EE5)c c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t ph$([char]0x1EA3)i b$([char]0x1EB1)ng {0}"
    }
    'release.draft-or-prerelease' = @{
        en = 'release must not be draft or prerelease'
        vi = "b$([char]0x1EA3)n ph$([char]0x00E1)t h$([char]0x00E0)nh kh$([char]0x00F4)ng $([char]0x0111)$([char]0x01B0)$([char]0x1EE3)c $([char]0x1EDF) tr$([char]0x1EA1)ng th$([char]0x00E1)i draft ho$([char]0x1EB7)c prerelease"
    }
    'release.tag-format' = @{
        en = 'release tag must match vMAJOR.MINOR.PATCH'
        vi = "th$([char]0x1EBB) ph$([char]0x00E1)t h$([char]0x00E0)nh ph$([char]0x1EA3)i kh$([char]0x1EDB)p $([char]0x0111)$([char]0x1ECB)nh d$([char]0x1EA1)ng vMAJOR.MINOR.PATCH"
    }
    'version.format' = @{
        en = 'version must start with v and match vMAJOR.MINOR.PATCH'
        vi = "phi$([char]0x00EA)n b$([char]0x1EA3)n ph$([char]0x1EA3)i b$([char]0x1EAF)t $([char]0x0111)$([char]0x1EA7)u b$([char]0x1EB1)ng v v$([char]0x00E0) kh$([char]0x1EDB)p $([char]0x0111)$([char]0x1ECB)nh d$([char]0x1EA1)ng vMAJOR.MINOR.PATCH"
    }
    'asset.installer-missing' = @{
        en = 'missing installer asset: {0}'
        vi = "thi$([char]0x1EBF)u t$([char]0x1EC7)p c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t: {0}"
    }
    'asset.installer-duplicate' = @{
        en = 'expected exactly one installer asset: {0}'
        vi = "ph$([char]0x1EA3)i c$([char]0x00F3) $([char]0x0111)$([char]0x00FA)ng m$([char]0x1ED9)t t$([char]0x1EC7)p c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t: {0}"
    }
    'asset.manifest-missing' = @{
        en = 'missing checksum manifest asset: {0}'
        vi = "thi$([char]0x1EBF)u t$([char]0x1EC7)p manifest checksum: {0}"
    }
    'asset.manifest-duplicate' = @{
        en = 'expected exactly one checksum manifest asset: {0}'
        vi = "ph$([char]0x1EA3)i c$([char]0x00F3) $([char]0x0111)$([char]0x00FA)ng m$([char]0x1ED9)t t$([char]0x1EC7)p manifest checksum: {0}"
    }
    'asset.download-url-missing' = @{
        en = 'release asset download URL is missing'
        vi = "thi$([char]0x1EBF)u URL t$([char]0x1EA3)i xu$([char]0x1ED1)ng c$([char]0x1EE7)a t$([char]0x1EC7)p ph$([char]0x00E1)t h$([char]0x00E0)nh"
    }
    'manifest.asset-basename' = @{
        en = 'asset name must be a basename'
        vi = "t$([char]0x00EA)n t$([char]0x1EC7)p ph$([char]0x1EA3)i l$([char]0x00E0) t$([char]0x00EA)n c$([char]0x01A1) s$([char]0x1EDF), kh$([char]0x00F4)ng ch$([char]0x1EE9)a $([char]0x0111)$([char]0x01B0)$([char]0x1EDD)ng d$([char]0x1EAB)n"
    }
    'manifest.not-found' = @{
        en = 'checksum manifest does not exist: {0}'
        vi = "kh$([char]0x00F4)ng t$([char]0x00EC)m th$([char]0x1EA5)y t$([char]0x1EC7)p manifest checksum: {0}"
    }
    'manifest.entry-not-canonical' = @{
        en = 'checksum manifest entry is not canonical'
        vi = "m$([char]0x1EE5)c trong manifest checksum kh$([char]0x00F4)ng $([char]0x0111)$([char]0x00FA)ng $([char]0x0111)$([char]0x1ECB)nh d$([char]0x1EA1)ng chu$([char]0x1EA9)n"
    }
    'manifest.entry-count' = @{
        en = 'checksum manifest must contain exactly one checksum entry for {0}'
        vi = "manifest checksum ph$([char]0x1EA3)i ch$([char]0x1EE9)a $([char]0x0111)$([char]0x00FA)ng m$([char]0x1ED9)t m$([char]0x1EE5)c checksum cho {0}"
    }
    'process.exit-code' = @{
        en = 'installer process failed with exit code {0}'
        vi = "ti$([char]0x1EBF)n tr$([char]0x00EC)nh c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t th$([char]0x1EA5)t b$([char]0x1EA1)i v$([char]0x1EDB)i m$([char]0x00E3) tho$([char]0x00E1)t {0}"
    }
    'installer.not-found' = @{
        en = 'installer does not exist: {0}'
        vi = "kh$([char]0x00F4)ng t$([char]0x00EC)m th$([char]0x1EA5)y t$([char]0x1EC7)p c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t: {0}"
    }
    'uninstaller.must-equal' = @{
        en = 'uninstaller must equal {0}'
        vi = "t$([char]0x1EC7)p g$([char]0x1EE1) c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t ph$([char]0x1EA3)i b$([char]0x1EB1)ng {0}"
    }
    'uninstaller.not-found' = @{
        en = 'uninstaller does not exist: {0}'
        vi = "kh$([char]0x00F4)ng t$([char]0x00EC)m th$([char]0x1EA5)y t$([char]0x1EC7)p g$([char]0x1EE1) c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t: {0}"
    }
    'asset.download-failed' = @{
        en = 'asset download did not create {0}'
        vi = "qu$([char]0x00E1) tr$([char]0x00EC)nh t$([char]0x1EA3)i t$([char]0x1EC7)p kh$([char]0x00F4)ng t$([char]0x1EA1)o ra {0}"
    }
    'installer.sha256-mismatch' = @{
        en = 'installer SHA-256 does not match {0}'
        vi = "SHA-256 c$([char]0x1EE7)a t$([char]0x1EC7)p c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t kh$([char]0x00F4)ng kh$([char]0x1EDB)p v$([char]0x1EDB)i {0}"
    }
    'install.refused-executable-exists' = @{
        en = 'install refused because standard executable already exists: {0}'
        vi = "t$([char]0x1EEB) ch$([char]0x1ED1)i c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t v$([char]0x00EC) t$([char]0x1EC7)p th$([char]0x1EF1)c thi chu$([char]0x1EA9)n $([char]0x0111)$([char]0x00E3) t$([char]0x1ED3)n t$([char]0x1EA1)i: {0}"
    }
    'update.refused-executable-missing' = @{
        en = 'update refused because standard executable is missing: {0}'
        vi = "t$([char]0x1EEB) ch$([char]0x1ED1)i c$([char]0x1EAD)p nh$([char]0x1EAD)t v$([char]0x00EC) thi$([char]0x1EBF)u t$([char]0x1EC7)p th$([char]0x1EF1)c thi chu$([char]0x1EA9)n: {0}"
    }
    'update.refused-running-process' = @{
        en = 'close VolumeControl before updating; bootstrap will not force-terminate it'
        vi = "h$([char]0x00E3)y $([char]0x0111)$([char]0x00F3)ng VolumeControl tr$([char]0x01B0)$([char]0x1EDB)c khi c$([char]0x1EAD)p nh$([char]0x1EAD)t; bootstrap s$([char]0x1EBD) kh$([char]0x00F4)ng bu$([char]0x1ED9)c k$([char]0x1EBF)t th$([char]0x00FA)c ti$([char]0x1EBF)n tr$([char]0x00EC)nh"
    }
    'install.success' = @{
        en = 'installed VolumeControl {0} into {1}'
        vi = "$([char]0x0110)$([char]0x00E3) c$([char]0x00E0)i VolumeControl {0} v$([char]0x00E0)o {1}"
    }
    'update.success' = @{
        en = 'updated VolumeControl {0} at {1}'
        vi = "$([char]0x0110)$([char]0x00E3) c$([char]0x1EAD)p nh$([char]0x1EAD)t VolumeControl {0} t$([char]0x1EA1)i {1}"
    }
    'update.current' = @{
        en = 'VolumeControl {0} is already installed; nothing to update'
        vi = "VolumeControl {0} $([char]0x0111)$([char]0x00E3) $([char]0x0111)$([char]0x01B0)$([char]0x1EE3)c c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t, kh$([char]0x00F4)ng c$([char]0x1EA7)n c$([char]0x1EAD)p nh$([char]0x1EAD)t"
    }
    'install.no-downgrade' = @{
        en = 'installed version {0} is newer than selected release {1}; skipping downgrade'
        vi = "phi$([char]0x00EA)n b$([char]0x1EA3)n $([char]0x0111)$([char]0x00E3) c$([char]0x00E0)i {0} m$([char]0x1EDB)i h$([char]0x01A1)n b$([char]0x1EA3)n ph$([char]0x00E1)t h$([char]0x00E0)nh {1}; b$([char]0x1ECF) qua h$([char]0x1EA1) c$([char]0x1EA5)p"
    }
    'uninstall.success' = @{
        en = 'removed VolumeControl from {0}'
        vi = "$([char]0x0110)$([char]0x00E3) g$([char]0x1EE1) VolumeControl kh$([char]0x1ECF)i {0}"
    }
    'uninstall.nothing' = @{
        en = 'nothing installed at {0}'
        vi = "kh$([char]0x00F4)ng c$([char]0x00F3) b$([char]0x1EA3)n c$([char]0x00E0)i n$([char]0x00E0)o t$([char]0x1EA1)i {0}"
    }
    'progress.downloading' = @{
        en = '[1/3] downloading {0}...'
        vi = "[1/3] $([char]0x0110)ang t$([char]0x1EA3)i {0}..."
    }
    'progress.verifying' = @{
        en = '[2/3] verifying SHA-256 against {0}...'
        vi = "[2/3] X$([char]0x00E1)c minh SHA-256 v$([char]0x1EDB)i {0}..."
    }
    'progress.installing' = @{
        en = '[3/3] installing VolumeControl into {0}...'
        vi = "[3/3] $([char]0x0110)ang c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t VolumeControl v$([char]0x00E0)o {0}..."
    }
    'progress.checking' = @{
        en = 'checking existing installation at {0}...'
        vi = "Ki$([char]0x1EC3)m tra b$([char]0x1EA3)n c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t hi$([char]0x1EC7)n t$([char]0x1EA1)i t$([char]0x1EA1)i {0}..."
    }
    'component.updated' = @{
        en = 'updated component: {0}'
        vi = "$([char]0x0110)$([char]0x00E3) c$([char]0x1EAD)p nh$([char]0x1EAD)t th$([char]0x00E0)nh ph$([char]0x1EA7)n: {0}"
    }
    'component.identical' = @{
        en = 'component {0} is already identical; skipping'
        vi = "Th$([char]0x00E0)nh ph$([char]0x1EA7)n {0} $([char]0x0111)$([char]0x00E3) tr$([char]0x00F9)ng kh$([char]0x1EDB)p; b$([char]0x1ECF) qua"
    }
    'config.preserved' = @{
        en = 'user configuration preserved at {0}'
        vi = "C$([char]0x1EA5)u h$([char]0x00EC)nh ng$([char]0x01B0)$([char]0x1EDD)i d$([char]0x00F9)ng $([char]0x0111)$([char]0x01B0)$([char]0x1EE3)c gi$([char]0x1EEF) nguy$([char]0x00EA)n t$([char]0x1EA1)i {0}"
    }
    'lock.timeout' = @{
        en = 'could not acquire installation lock for {0} within 30 seconds'
        vi = "kh$([char]0x00F4)ng th$([char]0x1EC3) l$([char]0x1EA5)y kh$([char]0x00F3)a c$([char]0x00E0)i $([char]0x0111)$([char]0x1EB7)t cho {0} trong 30 gi$([char]0x00E2)y"
    }
    'stop.timeout' = @{
        en = 'could not close running VolumeControl within 15 seconds; please close it via the tray'
        vi = "kh$([char]0x00F4)ng th$([char]0x1EC3) $([char]0x0111)$([char]0x00F3)ng VolumeControl trong 15 gi$([char]0x00E2)y; vui l$([char]0x00F2)ng $([char]0x0111)$([char]0x00F3)ng qua khay h$([char]0x1EC7) th$([char]0x1ED1)ng"
    }
    'receipt.repaired' = @{
        en = 'repaired VolumeControl installation at {0}'
        vi = "$([char]0x0110)$([char]0x00E3) s$([char]0x1EED)a ch$([char]0x1EEF)a b$([char]0x1EA3)n c$([char]0x00E0)i VolumeControl t$([char]0x1EA1)i {0}"
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

function Acquire-InstallLock {
    param([Parameter(Mandatory = $true)][string] $InstallRoot)

    $lockFile = $InstallRoot + '.install.lock'
    $deadline = [System.DateTime]::UtcNow.AddSeconds(30)
    while ([System.DateTime]::UtcNow -lt $deadline) {
        try {
            $stream = [System.IO.File]::Open($lockFile, [System.IO.FileMode]::OpenOrCreate, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
            return [pscustomobject]@{
                Stream = $stream
                Path = $lockFile
            }
        } catch {
            Start-Sleep -Milliseconds 200
        }
    }
    throw (Get-VolumeControlMessage 'lock.timeout' $InstallRoot)
}

function Release-InstallLock {
    param($Lock)

    if ($null -ne $Lock -and $null -ne $Lock.Stream) {
        try {
            $Lock.Stream.Dispose()
        } catch {}
        try {
            if ([System.IO.File]::Exists($Lock.Path)) {
                [System.IO.File]::Delete($Lock.Path)
            }
        } catch {}
    }
}

function Stop-VolumeControlInstance {
    param([Parameter(Mandatory = $true)][string] $InstallRoot)

    $exePath = Join-Path $InstallRoot 'VolumeControl.exe'
    if (-not [System.IO.File]::Exists($exePath)) {
        return $false
    }

    $targetFullPath = [System.IO.Path]::GetFullPath($exePath)
    $processes = @(Get-Process -Name 'VolumeControl' -ErrorAction SilentlyContinue | Where-Object {
        try {
            $procPath = $_.Path
            if ([string]::IsNullOrWhiteSpace($procPath)) {
                $procPath = $_.MainModule.FileName
            }
            if (-not [string]::IsNullOrWhiteSpace($procPath)) {
                return [string]::Equals([System.IO.Path]::GetFullPath($procPath), $targetFullPath, [System.StringComparison]::OrdinalIgnoreCase)
            }
        } catch {}
        return $false
    })

    if ($processes.Count -eq 0) {
        return $false
    }

    # Step 1: signal graceful exit event
    try {
        $event = [System.Threading.EventWaitHandle]::OpenExisting('Local\VolumeControl.Exit')
        if ($null -ne $event) {
            [void]$event.Set()
            $event.Dispose()
        }
    } catch {}

    # Step 2: wait up to timeout seconds for process exit
    $timeoutSec = if ($null -ne $script:StopTimeoutSeconds) { [int]$script:StopTimeoutSeconds } else { 15 }
    $deadline = [System.DateTime]::UtcNow.AddSeconds($timeoutSec)
    while ([System.DateTime]::UtcNow -lt $deadline) {
        $surviving = @(Get-Process -Name 'VolumeControl' -ErrorAction SilentlyContinue | Where-Object {
            try {
                $procPath = $_.Path
                if ([string]::IsNullOrWhiteSpace($procPath)) {
                    $procPath = $_.MainModule.FileName
                }
                if (-not [string]::IsNullOrWhiteSpace($procPath)) {
                    return [string]::Equals([System.IO.Path]::GetFullPath($procPath), $targetFullPath, [System.StringComparison]::OrdinalIgnoreCase)
                }
            } catch {}
            return $false
        })
        if ($surviving.Count -eq 0) {
            return $true
        }
        if ($timeoutSec -gt 0 -and [System.DateTime]::UtcNow -gt $deadline.AddSeconds(-7)) {
            foreach ($p in $surviving) {
                try { [void]$p.CloseMainWindow() } catch {}
            }
        }
        if ($timeoutSec -gt 0) {
            Start-Sleep -Milliseconds 200
        }
    }

    throw (Get-VolumeControlMessage 'update.refused-running-process')
}

function Write-InstallReceipt {
    param(
        [Parameter(Mandatory = $true)][string] $InstallRoot,
        [Parameter(Mandatory = $true)][string] $Tag,
        [Parameter(Mandatory = $true)][string] $ExecutableSha256
    )

    $receiptPath = Join-Path $InstallRoot 'install-receipt.txt'
    $content = "volumecontrol-install-v1`n$Tag`n$ExecutableSha256`n"
    [System.IO.File]::WriteAllText($receiptPath, $content, [System.Text.UTF8Encoding]::new($false))
}

function Read-InstallReceipt {
    param([Parameter(Mandatory = $true)][string] $InstallRoot)

    $receiptPath = Join-Path $InstallRoot 'install-receipt.txt'
    if (-not [System.IO.File]::Exists($receiptPath)) {
        return $null
    }
    $lines = @([System.IO.File]::ReadAllLines($receiptPath))
    if ($lines.Count -lt 3) {
        return $null
    }
    if ($lines[0].Trim() -cne 'volumecontrol-install-v1') {
        return $null
    }
    $tag = $lines[1].Trim()
    $hash = $lines[2].Trim().ToLowerInvariant()
    if ($tag -notmatch '^v\d+\.\d+\.\d+$' -or $hash -notmatch '^[0-9a-f]{64}$') {
        return $null
    }
    return [pscustomobject]@{
        Format = $lines[0].Trim()
        Tag = $tag
        Sha256 = $hash
    }
}

function Parse-SemVerParts {
    param([string] $Raw)

    if ([string]::IsNullOrWhiteSpace($Raw)) { return $null }
    $clean = $Raw.Trim()
    if ($clean.StartsWith('v')) { $clean = $clean.Substring(1) }
    $parts = $clean.Split('.')
    if ($parts.Count -lt 3) { return $null }
    $maj = 0; $min = 0; $patch = 0
    if (-not [int]::TryParse($parts[0], [ref]$maj)) { return $null }
    if (-not [int]::TryParse($parts[1], [ref]$min)) { return $null }
    if (-not [int]::TryParse($parts[2], [ref]$patch)) { return $null }
    return [pscustomobject]@{ Major = $maj; Minor = $min; Patch = $patch }
}

function Compare-SemVer {
    param([string] $Left, [string] $Right)

    $l = Parse-SemVerParts -Raw $Left
    $r = Parse-SemVerParts -Raw $Right
    if ($null -eq $l -or $null -eq $r) { return 0 }
    if ($l.Major -ne $r.Major) { return $l.Major.CompareTo($r.Major) }
    if ($l.Minor -ne $r.Minor) { return $l.Minor.CompareTo($r.Minor) }
    return $l.Patch.CompareTo($r.Patch)
}

function Test-IntactInstallation {
    param(
        [Parameter(Mandatory = $true)][string] $InstallRoot,
        [Parameter(Mandatory = $true)][string] $ExpectedTag
    )

    $exePath = Join-Path $InstallRoot 'VolumeControl.exe'
    $uninstaller = Join-Path $InstallRoot 'uninstall.exe'
    if (-not [System.IO.File]::Exists($exePath) -or -not [System.IO.File]::Exists($uninstaller)) {
        return $false
    }
    $receipt = Read-InstallReceipt -InstallRoot $InstallRoot
    if ($null -eq $receipt) {
        return $false
    }
    if ($receipt.Tag -cne $ExpectedTag) {
        return $false
    }
    return (Test-FileSha256 -Path $exePath -ExpectedSha256 $receipt.Sha256)
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
    param([string] $Version, [string] $CustomBaseUrl)

    if (-not [string]::IsNullOrEmpty($Version) -and $Version -notmatch '^v\d+\.\d+\.\d+$') {
        throw (Get-VolumeControlMessage 'version.format')
    }

    $headers = @{ Accept = 'application/vnd.github+json' }
    $apiUrl = $script:VolumeControlReleaseApi
    if (-not [string]::IsNullOrWhiteSpace($CustomBaseUrl)) {
        $apiUrl = $CustomBaseUrl.TrimEnd('/')
    }

    if ([string]::IsNullOrEmpty($Version)) {
        $release = Invoke-RestMethod -Uri "$apiUrl/latest" -Headers $headers -ErrorAction Stop
    } else {
        $release = Invoke-RestMethod -Uri "$apiUrl/tags/$Version" -Headers $headers -ErrorAction Stop
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

function Compute-FileSha256 {
    param([Parameter(Mandatory = $true)][string] $Path)

    if (-not [System.IO.File]::Exists($Path)) { return '' }
    $stream = [System.IO.File]::Open($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([System.BitConverter]::ToString($sha256.ComputeHash($stream))).Replace('-', '').ToLowerInvariant()
    } finally {
        $sha256.Dispose()
        $stream.Dispose()
    }
}

function Wait-ForSuccessfulProcess {
    param(
        [Parameter(Mandatory = $true)][string] $FilePath,
        [string[]] $ArgumentList = @()
    )

    $startParameters = @{ FilePath = $FilePath; PassThru = $true }
    if ($ArgumentList.Count -gt 0) {
        $startParameters.ArgumentList = $ArgumentList
    }

    $process = Start-Process @startParameters
    $process.WaitForExit()
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
        Write-Host (Get-VolumeControlMessage 'progress.downloading' $releaseInstaller.Installer.name)
        Save-ReleaseAsset -Asset $releaseInstaller.Installer -DestinationPath $downloadedInstaller
        Save-ReleaseAsset -Asset $releaseInstaller.Manifest -DestinationPath $downloadedManifest
        Write-Host (Get-VolumeControlMessage 'progress.verifying' $script:ChecksumManifestName)
        $expectedSha256 = Read-ExpectedSha256 -ManifestPath $downloadedManifest -AssetName $releaseInstaller.Installer.name
        if (-not (Test-FileSha256 -Path $downloadedInstaller -ExpectedSha256 $expectedSha256)) {
            throw (Get-VolumeControlMessage 'installer.sha256-mismatch' $script:ChecksumManifestName)
        }

        Write-Host (Get-VolumeControlMessage 'progress.installing' $InstallRoot)
        Start-Installer -InstallerPath $downloadedInstaller -Silent:$Silent -InstallRoot $InstallRoot
        $completed = $true
    } finally {
        if ($completed) {
            Remove-Item -LiteralPath $downloadRoot -Recurse -Force -ErrorAction Stop
        }
    }
}

function Backup-OwnedFiles {
    param([string] $InstallRoot)

    $backupDir = Join-Path $env:TEMP ("VolumeControl-backup-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $backupDir -ErrorAction Stop | Out-Null
    foreach ($name in @('VolumeControl.exe', 'uninstall.exe', 'install-receipt.txt')) {
        $source = Join-Path $InstallRoot $name
        if ([System.IO.File]::Exists($source)) {
            Copy-Item -LiteralPath $source -Destination (Join-Path $backupDir $name) -Force
        }
    }
    return $backupDir
}

function Restore-OwnedFiles {
    param([string] $BackupDir, [string] $InstallRoot)

    if (-not [System.IO.Directory]::Exists($BackupDir)) { return }
    foreach ($name in @('VolumeControl.exe', 'uninstall.exe', 'install-receipt.txt')) {
        $backupItem = Join-Path $BackupDir $name
        if ([System.IO.File]::Exists($backupItem)) {
            Copy-Item -LiteralPath $backupItem -Destination (Join-Path $InstallRoot $name) -Force
        }
    }
}

function Remove-RunKeyIfOwned {
    param([string] $InstalledExecutable)

    try {
        $runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
        $val = (Get-ItemProperty -Path $runKey -Name 'VolumeControl' -ErrorAction SilentlyContinue).VolumeControl
        if (-not [string]::IsNullOrWhiteSpace($val)) {
            if ($val.IndexOf($InstalledExecutable, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
                Remove-ItemProperty -Path $runKey -Name 'VolumeControl' -ErrorAction SilentlyContinue
            }
        }
    } catch {}
}

function Invoke-PreparedApply {
    param(
        [Parameter(Mandatory = $true)][string] $PreparedDir,
        [int] $ParentPid,
        [switch] $DoRestart,
        [string] $InstallRoot = (Get-InstallRoot)
    )

    $standardRoot = Assert-StandardInstallRoot -InstallRoot $InstallRoot
    $installedExecutable = Join-Path $standardRoot 'VolumeControl.exe'
    $lock = Acquire-InstallLock -InstallRoot $standardRoot

    try {
        # Signal readiness marker to host
        $readyMarker = Join-Path $PreparedDir 'handoff.ready'
        [System.IO.File]::WriteAllText($readyMarker, "ready`n", [System.Text.UTF8Encoding]::new($false))

        # Wait up to 15s for parent process exit
        if ($ParentPid -gt 0) {
            $deadline = [System.DateTime]::UtcNow.AddSeconds(15)
            while ([System.DateTime]::UtcNow -lt $deadline) {
                $proc = Get-Process -Id $ParentPid -ErrorAction SilentlyContinue
                if ($null -eq $proc -or $proc.HasExited) { break }
                Start-Sleep -Milliseconds 100
            }
        }

        # Locate installer in prepared dir
        $manifestPath = Join-Path $PreparedDir $script:ChecksumManifestName
        if (-not [System.IO.File]::Exists($manifestPath)) {
            throw "missing manifest in prepared directory: $manifestPath"
        }

        $installerFiles = @(Get-ChildItem -LiteralPath $PreparedDir -Filter 'VolumeControl_*_x64-setup.exe')
        if ($installerFiles.Count -ne 1) {
            throw "expected exactly one setup exe in prepared directory"
        }
        $installerPath = $installerFiles[0].FullName
        $expectedSha256 = Read-ExpectedSha256 -ManifestPath $manifestPath -AssetName $installerFiles[0].Name
        if (-not (Test-FileSha256 -Path $installerPath -ExpectedSha256 $expectedSha256)) {
            throw "installer hash mismatch in prepared directory"
        }

        $tagMatch = [regex]::Match($installerFiles[0].Name, '^VolumeControl_(.+)_x64-setup\.exe$')
        $versionTag = if ($tagMatch.Success) { 'v' + $tagMatch.Groups[1].Value } else { 'v0.0.0' }

        $backupDir = Backup-OwnedFiles -InstallRoot $standardRoot
        try {
            Start-Installer -InstallerPath $installerPath -Silent -InstallRoot $standardRoot
        } catch {
            Restore-OwnedFiles -BackupDir $backupDir -InstallRoot $standardRoot
            throw
        } finally {
            Remove-Item -LiteralPath $backupDir -Recurse -Force -ErrorAction SilentlyContinue
        }

        $installedHash = Compute-FileSha256 -Path $installedExecutable
        if (-not [string]::IsNullOrEmpty($installedHash)) {
            Write-InstallReceipt -InstallRoot $standardRoot -Tag $versionTag -ExecutableSha256 $installedHash
        }

        # Write result.exit-code atomically
        $resultTmp = Join-Path $PreparedDir 'result.exit-code.tmp'
        $resultFinal = Join-Path $PreparedDir 'result.exit-code'
        [System.IO.File]::WriteAllText($resultTmp, "0`n", [System.Text.UTF8Encoding]::new($false))
        Move-Item -LiteralPath $resultTmp -Destination $resultFinal -Force

        if ($DoRestart -and [System.IO.File]::Exists($installedExecutable)) {
            Start-Process -FilePath $installedExecutable -ArgumentList @('--background')
        }
    } finally {
        Release-InstallLock -Lock $lock
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
        [string] $InstallRoot = (Get-InstallRoot),
        [string] $PreparedDir,
        [int] $ParentPid = 0,
        [switch] $DoRestart,
        [string] $CustomBaseUrl
    )

    if (-not [string]::IsNullOrWhiteSpace($PreparedDir)) {
        Invoke-PreparedApply -PreparedDir $PreparedDir -ParentPid $ParentPid -DoRestart:$DoRestart -InstallRoot $InstallRoot
        return
    }

    $standardRoot = Assert-StandardInstallRoot -InstallRoot $InstallRoot
    $installedExecutable = Join-Path $standardRoot 'VolumeControl.exe'
    $uninstaller = Join-Path $standardRoot 'uninstall.exe'

    if ($Action -ceq 'uninstall') {
        if (-not [System.IO.File]::Exists($installedExecutable) -and -not [System.IO.File]::Exists($uninstaller)) {
            Write-Host (Get-VolumeControlMessage 'uninstall.nothing' $standardRoot)
            return
        }

        $lock = Acquire-InstallLock -InstallRoot $standardRoot
        try {
            [void](Stop-VolumeControlInstance -InstallRoot $standardRoot)
            if ([System.IO.File]::Exists($uninstaller)) {
                Start-Uninstaller -UninstallerPath $uninstaller -Silent:$Silent -InstallRoot $standardRoot
            } else {
                if ([System.IO.File]::Exists($installedExecutable)) {
                    throw (Get-VolumeControlMessage 'uninstaller.not-found' $uninstaller)
                }
            }
            Remove-RunKeyIfOwned -InstalledExecutable $installedExecutable
            $receiptPath = Join-Path $standardRoot 'install-receipt.txt'
            if ([System.IO.File]::Exists($receiptPath)) {
                Remove-Item -LiteralPath $receiptPath -Force -ErrorAction SilentlyContinue
            }
            Write-Host (Get-VolumeControlMessage 'uninstall.success' $standardRoot)
        } finally {
            Release-InstallLock -Lock $lock
        }
        return
    }

    if ($null -eq $Release) {
        $Release = Resolve-VolumeControlRelease -Version $Version -CustomBaseUrl $CustomBaseUrl
    } else {
        $Release = Assert-EligibleRelease -Release $Release
    }

    $selectedVersion = $Release.tag_name.Substring(1)
    $lock = Acquire-InstallLock -InstallRoot $standardRoot

    try {
        if ([System.IO.File]::Exists($installedExecutable)) {
            $installedVersionInfo = (Get-Item -LiteralPath $installedExecutable).VersionInfo.FileVersion
            $installedVersion = if ($null -ne $installedVersionInfo) { $installedVersionInfo.Trim() } else { '0.0.0' }

            $cmp = Compare-SemVer -Left $installedVersion -Right $selectedVersion
            if ($cmp -gt 0) {
                # Installed version is newer: keep it, report no downgrade, exit 0
                Write-Host (Get-VolumeControlMessage 'install.no-downgrade' $installedVersion $Release.tag_name)
                return
            }
            if ($cmp -eq 0 -and (Test-IntactInstallation -InstallRoot $standardRoot -ExpectedTag $Release.tag_name)) {
                # Same version intact: no-op, exit 0
                Write-Host (Get-VolumeControlMessage 'update.current' $selectedVersion)
                return
            }
        }

        # Upgrade or fresh install
        $wasRunning = Stop-VolumeControlInstance -InstallRoot $standardRoot
        $backupDir = Backup-OwnedFiles -InstallRoot $standardRoot

        try {
            Invoke-VerifiedInstaller -Release $Release -InstallRoot $standardRoot -Silent:$Silent
        } catch {
            Restore-OwnedFiles -BackupDir $backupDir -InstallRoot $standardRoot
            throw
        } finally {
            Remove-Item -LiteralPath $backupDir -Recurse -Force -ErrorAction SilentlyContinue
        }

        $installedHash = Compute-FileSha256 -Path $installedExecutable
        if (-not [string]::IsNullOrEmpty($installedHash)) {
            Write-InstallReceipt -InstallRoot $standardRoot -Tag $Release.tag_name -ExecutableSha256 $installedHash
        }

        if ($wasRunning -or $DoRestart) {
            if ([System.IO.File]::Exists($installedExecutable)) {
                Start-Process -FilePath $installedExecutable -ArgumentList @('--background')
            }
        }

        if ($Action -ceq 'install') {
            Write-Host (Get-VolumeControlMessage 'install.success' $selectedVersion $standardRoot)
        } else {
            Write-Host (Get-VolumeControlMessage 'update.success' $selectedVersion $standardRoot)
        }
    } finally {
        Release-InstallLock -Lock $lock
    }
}

if ($StopOnly) {
    $installRoot = Get-InstallRoot
    [void](Stop-VolumeControlInstance -InstallRoot $installRoot)
    exit 0
}

if ($MyInvocation.InvocationName -ne '.') {
    Invoke-VolumeControlAction -Action $Action -Version $Version -Silent:$Silent -PreparedDir $PreparedDirectory -ParentPid $ParentProcessId -DoRestart:$Restart -CustomBaseUrl $ReleaseBaseUrl
}
