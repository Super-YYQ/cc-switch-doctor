$ErrorActionPreference = "Stop"
$vs = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools"
$vcvars = Join-Path $vs "VC\Auxiliary\Build\vcvars64.bat"
if (-not (Test-Path $vcvars)) { throw "vcvars64.bat not found: $vcvars" }

$temp = [IO.Path]::GetTempFileName()
cmd /c "`"$vcvars`" && set" > $temp
Get-Content $temp | ForEach-Object {
  if ($_ -match "^(.*?)=(.*)$") {
    [System.Environment]::SetEnvironmentVariable($matches[1], $matches[2], "Process")
  }
}
Remove-Item $temp -Force

$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
Set-Location "C:\dev\cc-switch-doctor"

$cmd = $args -join " "
if (-not $cmd) { $cmd = "cargo test --manifest-path src-tauri\Cargo.toml" }
Write-Host "RUN: $cmd"
Invoke-Expression $cmd
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
