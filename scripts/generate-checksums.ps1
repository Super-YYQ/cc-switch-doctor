param(
  [Parameter(Mandatory = $true)]
  [string]$AssetsDir
)

$ErrorActionPreference = "Stop"
$out = Join-Path $AssetsDir "SHA256SUMS.txt"
if (Test-Path $out) { Remove-Item -Force $out }

$lines = @()
Get-ChildItem -Path $AssetsDir -File | Where-Object {
  $_.Name -ne "SHA256SUMS.txt"
} | Sort-Object Name | ForEach-Object {
  $hash = (Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLower()
  $lines += "$hash  $($_.Name)"
}

$lines -join "`n" | Set-Content -Encoding ascii -NoNewline $out
Add-Content -Encoding ascii $out "`n"
Write-Host "Wrote $out"
Get-Content $out
