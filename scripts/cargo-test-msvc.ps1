$ErrorActionPreference = "Stop"
$vs = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools"
$vcvars = Join-Path $vs "VC\Auxiliary\Build\vcvars64.bat"
if (-not (Test-Path $vcvars)) { throw "vcvars64.bat not found: $vcvars" }

# Import vcvars environment into current PowerShell session
$temp = [IO.Path]::GetTempFileName()
cmd /c "`"$vcvars`" && set" > $temp
Get-Content $temp | ForEach-Object {
  if ($_ -match "^(.*?)=(.*)$") {
    $name = $matches[1]
    $value = $matches[2]
    [System.Environment]::SetEnvironmentVariable($name, $value, "Process")
  }
}
Remove-Item $temp -Force

$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
Set-Location "C:\dev\cc-switch-doctor"
Write-Host "cl:" (Get-Command cl -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source)
Write-Host "link:" (Get-Command link -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source)
Write-Host "rustc:" (rustc --version)
cargo test --manifest-path src-tauri\Cargo.toml --lib
