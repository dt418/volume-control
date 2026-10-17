# Live verification of the product-page commands against the published
# v0.1.5 release (irm flow, then the curl flow with -Action update).
$ErrorActionPreference = 'Stop'
$base = 'https://github.com/dt418/volume-control/releases/latest/download'
$installRoot = Join-Path $env:LOCALAPPDATA 'VolumeControl'

Write-Output "== irm flow: install =="
& ([scriptblock]::Create((irm "$base/Install-VolumeControl.ps1"))) -Action install -Silent
Write-Output "install_exit=$LASTEXITCODE"

$exe = Join-Path $installRoot 'VolumeControl.exe'
Write-Output "installed_exe_exists=$(Test-Path $exe)"
if (Test-Path $exe) {
    Write-Output ("installed_file_version=" + (Get-Item $exe).VersionInfo.FileVersion)
}

Write-Output "== curl flow: download, verify, update =="
$root = Join-Path $env:TEMP ('VolumeControl-curl-check-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $root | Out-Null
curl.exe -fsSL "$base/Install-VolumeControl.ps1" -o "$root\Install-VolumeControl.ps1"
curl.exe -fsSL "$base/SHA256SUMS.txt" -o "$root\SHA256SUMS.txt"
$expected = ((Get-Content "$root\SHA256SUMS.txt" | Where-Object { $_ -like '*Install-VolumeControl.ps1' }) -split '\s+')[0]
$actual = (Get-FileHash "$root\Install-VolumeControl.ps1" -Algorithm SHA256).Hash
Write-Output "script_sha_match=$($actual -ieq $expected)"
& "$root\Install-VolumeControl.ps1" -Action update -Silent
Write-Output "update_exit=$LASTEXITCODE"

Write-Output "== manifest covers installer =="
$installerLine = Get-Content "$root\SHA256SUMS.txt" | Where-Object { $_ -like '*_x64-setup.exe' }
Write-Output "installer_manifest_entry=$([bool]$installerLine)"
Remove-Item -Recurse -Force $root
Write-Output "done"
