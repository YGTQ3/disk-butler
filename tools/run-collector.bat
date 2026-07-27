@echo off
rem DiskButler Rule Collector launcher. ASCII only - works on every PC.
rem Chinese guide: https://github.com/YGTQ3/disk-butler/tree/main/tools
cd /d "%~dp0"
if not exist "collect-rules.ps1" (
    echo [ERROR] collect-rules.ps1 not found here.
    echo Download: https://github.com/YGTQ3/disk-butler/tree/main/tools
    pause
    exit /b 1
)
echo ============================================
echo   DiskButler  Rule Collector
echo   Safe: NO delete, NO change, NO network.
echo   Report file goes to your Desktop.
echo ============================================
echo.
echo   [1] Basic scan  - about 1 min  (recommended)
echo   [2] Full scan   - 5 to 10 min, also lists big folders on D: etc.
echo.
set MODE=
set /p MODE=Type 1 or 2, then press Enter (Enter only = 1):
echo.
if "%MODE%"=="2" (
    echo Mode: Full scan
    powershell -NoProfile -ExecutionPolicy Bypass -File "collect-rules.ps1" -IncludeDrives
) else (
    echo Mode: Basic scan
    powershell -NoProfile -ExecutionPolicy Bypass -File "collect-rules.ps1"
)
echo.
echo Done! Report is on your Desktop: diskbutler-rule-report-*.md
echo Please open it with Notepad and check it before sharing.
pause
