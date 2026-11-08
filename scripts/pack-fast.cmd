@echo off
rem 一键打包:fast 构建 + dist 三件套 + zip(页面文件扩容后重启跑这个即可)
rem 用法:双击运行,或在 cmd 里执行 scripts\pack-fast.cmd
cd /d C:\workspace\MTerm
set CARGO_BUILD_JOBS=1
set CARGO_PROFILE_FAST_CODEGEN_UNITS=256
echo [1/3] building (fast profile, ~15-20 min)...
cargo build --profile fast
if errorlevel 1 (
    echo BUILD FAILED - 内存不足时请确认已重启(32GB 页面文件生效)
    pause
    exit /b 1
)
echo [2/3] staging...
if exist dist\MTerm-v0.7.3-win-x64 rmdir /s /q dist\MTerm-v0.7.3-win-x64
mkdir dist\MTerm-v0.7.3-win-x64
copy /y target\fast\MTerm.exe dist\MTerm-v0.7.3-win-x64\ >nul
copy /y CHANGELOG.md dist\MTerm-v0.7.3-win-x64\ >nul
copy /y README.md dist\MTerm-v0.7.3-win-x64\ >nul
echo [3/3] zipping...
powershell -NoProfile -Command "if (Test-Path 'dist\MTerm-v0.7.3-win-x64.zip') { Remove-Item 'dist\MTerm-v0.7.3-win-x64.zip' }; Compress-Archive -Path 'dist\MTerm-v0.7.3-win-x64' -DestinationPath 'dist\MTerm-v0.7.3-win-x64.zip'"
echo PACK DONE: dist\MTerm-v0.7.3-win-x64.zip
pause
