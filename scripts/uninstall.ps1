param(
    [string]$InstallRoot = (Join-Path $env:LOCALAPPDATA "Steward"),
    [switch]$RemoveData
)

$ErrorActionPreference = "Stop"
$root = [System.IO.Path]::GetFullPath($InstallRoot)
$localAppData = [System.IO.Path]::GetFullPath($env:LOCALAPPDATA)
if (-not $root.StartsWith($localAppData, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "InstallRoot must remain inside LOCALAPPDATA"
}

Unregister-ScheduledTask -TaskName "Steward Daemon" -Confirm:$false -ErrorAction SilentlyContinue
Get-Process -Name "steward-daemon" -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($root, [System.StringComparison]::OrdinalIgnoreCase) } | Stop-Process -Force

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$marker = Join-Path $root "path-entry.marker"
if (Test-Path -LiteralPath $marker) {
    $separatorLength = [int](Get-Content -Raw -LiteralPath $marker)
    $suffixLength = $root.Length + $separatorLength
    if ($userPath.EndsWith($root, [System.StringComparison]::OrdinalIgnoreCase)) {
        [Environment]::SetEnvironmentVariable("Path", $userPath.Substring(0, $userPath.Length - $suffixLength), "User")
    }
}
if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }

if ($RemoveData) {
    $dataRoot = [System.IO.Path]::GetFullPath((Join-Path $env:USERPROFILE ".steward"))
    if ($dataRoot -ne [System.IO.Path]::GetFullPath($env:USERPROFILE)) {
        Remove-Item -LiteralPath $dataRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
Write-Output "uninstalled=$root"
