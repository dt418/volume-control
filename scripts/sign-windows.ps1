# Sign Windows binaries with Authenticode (signtool).
#
#   scripts/sign-windows.ps1 -Files setup.exe,volumecontrol-host.exe
#
# Credentials (first match wins):
#   WINDOWS_CERT_PFX_BASE64 + WINDOWS_CERT_PASSWORD  (base64 .pfx, CI)
#   WINDOWS_CERT_THUMBPRINT                          (cert already in store)
# No credentials or no signtool: logs a skip and exits 0 so local packaging
# works unsigned; a public release must run with credentials.

param(
    [Parameter(Mandatory = $true)][string[]]$Files,
    [string]$SignTool,
    [switch]$VerifyOnly
)

$ErrorActionPreference = 'Stop'
$requireSigning = $env:REQUIRE_SIGNING -eq 'true'

function Resolve-SignTool {
    if ($SignTool) { return $SignTool }
    $onPath = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }
    $kits = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Directory `
        -ErrorAction SilentlyContinue | Sort-Object Name -Descending
    foreach ($kit in $kits) {
        $candidate = Join-Path $kit.FullName "x64\signtool.exe"
        if (Test-Path $candidate) { return $candidate }
    }
    return $null
}

$tool = Resolve-SignTool
if (-not $tool) {
    if ($requireSigning) { throw 'REQUIRE_SIGNING=true but signtool was not found' }
    Write-Host 'signing skipped (signtool not found)'
    exit 0
}

if ($VerifyOnly) {
    foreach ($file in $Files) {
        & $tool verify /pa $file
        if ($LASTEXITCODE -ne 0) { throw "signature verification failed: $file" }
    }
    exit 0
}

$pfx = $env:WINDOWS_CERT_PFX_BASE64
$password = $env:WINDOWS_CERT_PASSWORD
$thumbprint = $env:WINDOWS_CERT_THUMBPRINT
if (-not $pfx -and -not $thumbprint) {
    if ($requireSigning) { throw 'REQUIRE_SIGNING=true but no WINDOWS_CERT_* credentials are set' }
    Write-Host 'signing skipped (no WINDOWS_CERT_* credentials)'
    exit 0
}

$common = @('sign', '/fd', 'sha256', '/tr', 'http://timestamp.digicert.com', '/td', 'sha256')
$temporary = $null
try {
    if ($pfx) {
        if (-not $password) { throw 'WINDOWS_CERT_PASSWORD is required with WINDOWS_CERT_PFX_BASE64' }
        $temporary = Join-Path ([System.IO.Path]::GetTempPath()) ("volumecontrol-{0}.pfx" -f [guid]::NewGuid())
        [System.IO.File]::WriteAllBytes($temporary, [Convert]::FromBase64String($pfx))
        foreach ($file in $Files) {
            & $tool @common /f $temporary /p $password $file
            if ($LASTEXITCODE -ne 0) { throw "signtool failed for $file" }
            Write-Host "signed $file"
        }
    } else {
        foreach ($file in $Files) {
            & $tool @common /sha1 $thumbprint $file
            if ($LASTEXITCODE -ne 0) { throw "signtool failed for $file" }
            Write-Host "signed $file"
        }
    }
    $allowUntrusted = $env:WINDOWS_ALLOW_UNTRUSTED_SIGNATURE -eq 'true'
    foreach ($file in $Files) {
        & $tool verify /pa $file
        if ($LASTEXITCODE -ne 0) {
            if ($allowUntrusted) {
                Write-Warning "signature present but untrusted (test certificate): $file"
            } else {
                throw "signature verification failed: $file"
            }
        }
    }
} finally {
    if ($temporary) { Remove-Item $temporary -Force -ErrorAction SilentlyContinue }
}
exit 0
