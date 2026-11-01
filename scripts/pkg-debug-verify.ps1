# debug 版验证包(临时;正式包仍走 scripts/pkg-verify.ps1 的 release 流程)
$ErrorActionPreference = "Stop"
Set-Location "C:\workspace\MTerm"

$exe = "target\debug\MTerm.exe"
if (-not (Test-Path $exe)) { throw "debug exe not found: $exe" }

$objdump = "C:\msys64\mingw64\bin\objdump.exe"
if (-not (Test-Path $objdump)) { throw "objdump not found: $objdump" }

$deps = & $objdump -p $exe | Select-String "DLL Name:" | ForEach-Object {
    ($_.Line -split "DLL Name:")[1].Trim()
} | Sort-Object -Unique

$stage = "mterm-debug-verify"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Path $stage | Out-Null

Copy-Item $exe (Join-Path $stage "MTerm.exe")
foreach ($d in $deps) {
    $sys = Join-Path "$env:SystemRoot\System32" $d
    $local = Join-Path "C:\msys64\mingw64\bin" $d
    if (-not (Test-Path $sys) -and (Test-Path $local)) {
        Copy-Item $local (Join-Path $stage $d)
        Write-Host "bundled: $d"
    }
}
if (Test-Path "lang") { Copy-Item -Recurse "lang" (Join-Path $stage "lang") }

if (Test-Path "$stage.zip") { Remove-Item "$stage.zip" }
Compress-Archive -Path $stage -DestinationPath "$stage.zip"
$z = Get-Item "$stage.zip"
Write-Host ("zip: " + $z.FullName + " " + [math]::Round($z.Length/1MB, 1) + " MB")
Write-Host ("entries: " + (Get-ChildItem -Recurse $stage | Measure-Object).Count)
