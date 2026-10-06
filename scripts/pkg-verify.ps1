# Local verify package (port of scripts/_pkg_win.py; no Python on this box).
# Usage: powershell -ExecutionPolicy Bypass -File scripts\pkg-verify.ps1
# Output: repo-root meatshell-win-verify\ + meatshell-win-verify.zip
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$exe = "target\release\meatshell.exe"
if (-not (Test-Path $exe)) { throw "release exe not found: $exe (run cargo build --release first)" }

# 1) locate toolchain (llvm-mingw -> MSYS2 mingw64)
$objdump = $null
$mingwBin = $null
foreach ($c in @("$env:USERPROFILE\.cargo\bin\llvm-mingw\bin", "C:\msys64\mingw64\bin", "$env:USERPROFILE\msys64\mingw64\bin", "C:\llvm-mingw\bin")) {
    if (Test-Path (Join-Path $c "objdump.exe")) { $objdump = Join-Path $c "objdump.exe"; $mingwBin = $c; break }
}
if (-not $objdump) { throw "objdump.exe not found (llvm-mingw / msys64 mingw64)" }
Write-Host "toolchain: $mingwBin"

# 2) list dynamic dependencies of the exe
$deps = & $objdump -p $exe | Select-String "DLL Name:" | ForEach-Object {
    ($_.Line -split "DLL Name:")[1].Trim()
} | Sort-Object -Unique

# 3) bundle non-system DLLs (not present in System32 -> toolchain runtime)
$stage = "meatshell-win-verify"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Path $stage | Out-Null

Copy-Item $exe (Join-Path $stage "meatshell.exe")
foreach ($d in $deps) {
    $sys = Join-Path $env:SystemRoot "System32\$d"
    if (Test-Path $sys) { continue }
    $local = Join-Path $mingwBin $d
    if (Test-Path $local) {
        Copy-Item $local (Join-Path $stage $d)
        Write-Host "bundled: $d"
    } else {
        Write-Warning "missing DLL (not system, not in mingw64): $d"
    }
}

# 4) zip
if (Test-Path "$stage.zip") { Remove-Item "$stage.zip" }
Compress-Archive -Path $stage -DestinationPath "$stage.zip"
Write-Host "zip: $stage.zip"

# 5) verify zip entries
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::OpenRead((Resolve-Path "$stage.zip"))
Write-Host "-- zip entries --"
$zip.Entries | ForEach-Object { Write-Host ("  {0}  {1}" -f $_.FullName, $_.Length) }
$zip.Dispose()
Write-Host "package done."
