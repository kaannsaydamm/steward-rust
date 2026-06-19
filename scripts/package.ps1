param(
    [string]$OutputDirectory = "dist"
)

$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$output = [System.IO.Path]::GetFullPath((Join-Path $repo $OutputDirectory))
$stage = Join-Path $output "steward-windows-x64"

$repoPrefix = $repo.TrimEnd('\') + '\'
if (-not $output.StartsWith($repoPrefix, [System.StringComparison]::OrdinalIgnoreCase)) { throw "Output directory must be inside the repository" }
if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null

Push-Location $repo
try {
    cargo build --release -p steward-cli -p steward-daemon
    if ($LASTEXITCODE -ne 0) { throw "Release build failed" }
} finally {
    Pop-Location
}

Copy-Item -LiteralPath (Join-Path $repo "target\release\steward-cli.exe") -Destination (Join-Path $stage "steward.exe")
Copy-Item -LiteralPath (Join-Path $repo "target\release\steward-daemon.exe") -Destination $stage
Copy-Item -LiteralPath (Join-Path $repo "README.md") -Destination $stage
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "install.ps1") -Destination $stage
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "uninstall.ps1") -Destination $stage
'{"retention_days":30,"max_completed_workflows":200}' | Set-Content -LiteralPath (Join-Path $stage "config.example.json") -Encoding utf8NoBOM

$manifest = Get-ChildItem -LiteralPath $stage -File | Sort-Object Name | ForEach-Object {
    $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $($_.Name)"
}
$manifest | Set-Content -LiteralPath (Join-Path $stage "SHA256SUMS") -Encoding ascii

$archive = Join-Path $output "steward-windows-x64.zip"
if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive -Force }
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $archive -CompressionLevel Optimal
Write-Output $archive
