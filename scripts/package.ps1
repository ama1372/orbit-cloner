$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$releaseRoot = Join-Path $projectRoot 'release'
New-Item -ItemType Directory -Force -Path $releaseRoot | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot 'src-tauri\target\release\orbit-cloner.exe') -Destination (Join-Path $releaseRoot 'Orbit Cloner.exe')
Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md'),(Join-Path $projectRoot 'LICENSE') -Destination $releaseRoot
$archive = Join-Path $releaseRoot 'orbit-cloner-windows-x64.zip'
# Explicit file list: never package portable-data or vendor runtime copies.
Compress-Archive -LiteralPath (Join-Path $releaseRoot 'Orbit Cloner.exe'),(Join-Path $releaseRoot 'README.md'),(Join-Path $releaseRoot 'LICENSE') -DestinationPath $archive -Force
Get-Item -LiteralPath $archive
