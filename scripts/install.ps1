param([string]$InstallRoot = (Join-Path $env:LOCALAPPDATA "Steward"))

$ErrorActionPreference = "Stop"
$source = $PSScriptRoot
$root = [System.IO.Path]::GetFullPath($InstallRoot)
$cli = Join-Path $source "steward.exe"
$daemon = Join-Path $source "steward-daemon.exe"
$web = Join-Path $source "web-ui"
if (-not (Test-Path -LiteralPath $cli) -or -not (Test-Path -LiteralPath $daemon) -or -not (Test-Path -LiteralPath (Join-Path $web "index.html"))) {
    throw "Run install.ps1 from an extracted Steward release archive"
}

New-Item -ItemType Directory -Path $root -Force | Out-Null
Get-Process -Name "steward-daemon" -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($root, [System.StringComparison]::OrdinalIgnoreCase) } | Stop-Process -Force
Copy-Item -LiteralPath $cli -Destination (Join-Path $root "steward.exe") -Force
Copy-Item -LiteralPath $daemon -Destination (Join-Path $root "steward-daemon.exe") -Force
Copy-Item -LiteralPath $web -Destination (Join-Path $root "web-ui") -Recurse -Force
Copy-Item -LiteralPath (Join-Path $source "uninstall.ps1") -Destination $root -Force

$dataRoot = Join-Path $env:USERPROFILE ".steward"
New-Item -ItemType Directory -Path $dataRoot -Force | Out-Null
$config = Join-Path $dataRoot "config.json"
if (-not (Test-Path -LiteralPath $config)) {
    Copy-Item -LiteralPath (Join-Path $source "config.example.json") -Destination $config
}

Unregister-ScheduledTask -TaskName "Steward Daemon" -Confirm:$false -ErrorAction SilentlyContinue

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$pathEntries = @($userPath -split ';' | Where-Object { $_ })
if (-not ($pathEntries | Where-Object { [System.IO.Path]::GetFullPath($_) -eq $root })) {
    $separator = if ([string]::IsNullOrEmpty($userPath) -or $userPath.EndsWith(';')) { "" } else { ";" }
    $updatedPath = $userPath + $separator + $root
    [Environment]::SetEnvironmentVariable("Path", $updatedPath, "User")
    $separator.Length | Set-Content -LiteralPath (Join-Path $root "path-entry.marker") -Encoding ascii
}

Write-Output "installed=$root"
Write-Output "data=$dataRoot"
