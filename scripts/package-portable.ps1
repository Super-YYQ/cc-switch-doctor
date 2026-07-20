param(
  [Parameter(Mandatory = $true)]
  [string]$Version,
  [Parameter(Mandatory = $true)]
  [string]$ExePath,
  [Parameter(Mandatory = $true)]
  [string]$OutDir
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $ExePath)) {
  throw "EXE not found: $ExePath"
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$stage = Join-Path $OutDir "portable-stage"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force -Path $stage | Out-Null

Copy-Item $ExePath (Join-Path $stage "CC-Switch-Doctor.exe")
Copy-Item "LICENSE" (Join-Path $stage "LICENSE")
Copy-Item "PRIVACY.md" (Join-Path $stage "PRIVACY.md")

@"
CC Switch Doctor v$Version (Windows x64 portable)

This is an UNSIGNED build unless stated otherwise in the GitHub Release notes.
Windows SmartScreen may warn on first launch.

Usage:
1. Extract this ZIP.
2. Run CC-Switch-Doctor.exe
3. The app is stateless: it does not save keys, selections, or results.

Security:
- Read-only access to CC Switch database only
- Pure HTTP API tests (no AI CLI launch)
- Full API keys never leave Rust memory into the UI

Project: https://github.com/Super-YYQ/cc-switch-doctor
"@ | Set-Content -Encoding UTF8 (Join-Path $stage "README.txt")

$zipName = "CC-Switch-Doctor-v$Version-Windows-x64-portable.zip"
$zipPath = Join-Path $OutDir $zipName
if (Test-Path $zipPath) { Remove-Item -Force $zipPath }

Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zipPath -Force
Write-Host "Portable package: $zipPath"
Write-Output $zipPath
