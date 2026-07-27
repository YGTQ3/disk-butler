@echo off
rem DiskButler Rule Collector launcher - double-click to run.
rem Requires collect-rules.ps1 in the same folder.
cd /d "%~dp0"
if not exist "collect-rules.ps1" (
    echo [ERROR] collect-rules.ps1 not found in this folder.
    echo Please download it from:
    echo https://github.com/YGTQ3/disk-butler/tree/main/tools
    pause
    exit /b 1
)
echo ============================================
echo  DiskButler Rule Collector
echo  - Nothing will be deleted or modified.
echo  - No network access. Report goes to Desktop.
echo ============================================
echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "collect-rules.ps1" %*
echo.
echo Finished. The report files are on your Desktop.
echo Please open the .md file and review it before sharing.
pause
