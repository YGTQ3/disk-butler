@echo off
chcp 65001 >nul 2>&1
title C盘管家 - WebView2 环境诊断

:: 自动请求管理员权限
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo 正在请求管理员权限...
    powershell -Command "Start-Process '%~f0' -Verb RunAs" 2>nul
    exit /b
)

setlocal enabledelayedexpansion

set "REPORT=%USERPROFILE%\Desktop\disk-butler-webview2-report.txt"

echo ============================================ > "%REPORT%"
echo   C盘管家 WebView2 环境诊断报告            >> "%REPORT%"
echo   生成时间: %date% %time%                   >> "%REPORT%"
echo ============================================ >> "%REPORT%"
echo. >> "%REPORT%"

:: ========== 1. 系统信息 ==========
echo [1/7] 正在收集系统信息（可能需要30秒）...
echo [1/7] 系统信息 >> "%REPORT%"
echo -------------------------------------------- >> "%REPORT%"
systeminfo | findstr /C:"OS 名称" /C:"OS 版本" /C:"系统类型" /C:"处理器" >> "%REPORT%" 2>nul
if errorlevel 1 (
    systeminfo | findstr /C:"OS Name" /C:"OS Version" /C:"System Type" /C:"Processor" >> "%REPORT%" 2>nul
)
echo. >> "%REPORT%"

:: ========== 2. 当前用户 ==========
echo [2/7] 正在检查当前用户...
echo [2/7] 当前用户 >> "%REPORT%"
echo -------------------------------------------- >> "%REPORT%"
echo 用户名: %USERNAME% >> "%REPORT%"
echo 用户目录: %USERPROFILE% >> "%REPORT%"
echo 是否管理员: 是 >> "%REPORT%"
echo. >> "%REPORT%"

:: ========== 3. WebView2 文件检查 ==========
echo [3/7] 正在检查 WebView2 文件目录...
echo [3/7] WebView2 文件目录检查 >> "%REPORT%"
echo -------------------------------------------- >> "%REPORT%"

set "WV2_SYS=C:\Program Files (x86)\Microsoft\EdgeWebView"
set "WV2_SYS64=C:\Program Files\Microsoft\EdgeWebView"
set "WV2_USER=%LOCALAPPDATA%\Microsoft\EdgeWebView"

echo 系统级 (x86): %WV2_SYS% >> "%REPORT%"
if exist "%WV2_SYS%" (
    echo   [存在] >> "%REPORT%"
    dir /b "%WV2_SYS%" >> "%REPORT%" 2>nul
) else (
    echo   [不存在] >> "%REPORT%"
)
echo. >> "%REPORT%"

echo 系统级 (x64): %WV2_SYS64% >> "%REPORT%"
if exist "%WV2_SYS64%" (
    echo   [存在] >> "%REPORT%"
    dir /b "%WV2_SYS64%" >> "%REPORT%" 2>nul
) else (
    echo   [不存在] >> "%REPORT%"
)
echo. >> "%REPORT%"

echo 用户级: %WV2_USER% >> "%REPORT%"
if exist "%WV2_USER%" (
    echo   [存在] >> "%REPORT%"
    dir /b "%WV2_USER%" >> "%REPORT%" 2>nul
) else (
    echo   [不存在] >> "%REPORT%"
)
echo. >> "%REPORT%"

:: ========== 3b. WebView2 x64 实体组件检查（反馈 P 修正判据） ==========
:: 关键：注册表 pv 存在 ≠ x64 Runtime 可用——必须查实体 exe 并确认位深。
:: 两种布局都认：① Application\<版本>\msedgewebview2.exe（读 PE 头判位深）
::              ② Application\<版本>\EBWebView\x64\msedgewebview2.exe（旧版固定版）
echo [3b/7] 正在检查 WebView2 x64 实体组件...
echo [3b/7] WebView2 x64 实体组件检查（pv存在不等于x64可用） >> "%REPORT%"
echo -------------------------------------------- >> "%REPORT%"

set "WV2_X64_FOUND=0"
powershell -NoProfile -Command "$found=$false; foreach($r in @('%WV2_SYS%','%WV2_USER%')){ $a=Join-Path $r 'Application'; if(Test-Path $a){ foreach($v in Get-ChildItem $a -Directory){ $tag=''; $p1=Join-Path $v.FullName 'EBWebView\x64\msedgewebview2.exe'; $p2=Join-Path $v.FullName 'msedgewebview2.exe'; if(Test-Path $p1){ $tag='[x64 正常-旧版布局]'; $found=$true } elseif(Test-Path $p2){ try{ $fs=[IO.File]::OpenRead($p2); $br=New-Object IO.BinaryReader($fs); $fs.Position=0x3C; $pe=$br.ReadInt32(); $fs.Position=$pe+4; $m=$br.ReadUInt16(); $fs.Close(); if($m -eq 0x8664){ $tag='[x64 正常]'; $found=$true } else { $tag='[仅 x86]' } } catch { $tag='[读取失败]' } } else { $tag='[无 exe]' }; Write-Output ($tag+' '+$v.FullName) } } }; if($found){ exit 0 } else { exit 1 }" >> "%REPORT%" 2>nul
if not errorlevel 1 set "WV2_X64_FOUND=1"
if "%WV2_X64_FOUND%"=="0" (
    echo   [未找到任何 x64 组件] >> "%REPORT%"
)
echo. >> "%REPORT%"

:: ========== 4. WebView2 注册表检查 ==========
echo [4/7] 正在检查 WebView2 注册表...
echo [4/7] WebView2 注册表检查 >> "%REPORT%"
echo -------------------------------------------- >> "%REPORT%"

set "REG_GUID={F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

set "WV2_PV="
for /f "tokens=3" %%a in ('reg query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\%REG_GUID%" /v pv 2^>nul ^| findstr /i "pv"') do set "WV2_PV=%%a"
if "%WV2_PV%"=="" (
    for /f "tokens=3" %%a in ('reg query "HKCU\SOFTWARE\Microsoft\EdgeUpdate\Clients\%REG_GUID%" /v pv 2^>nul ^| findstr /i "pv"') do set "WV2_PV=%%a"
)

echo --- HKLM (系统级) --- >> "%REPORT%"
reg query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\%REG_GUID%" /v pv >> "%REPORT%" 2>nul
if errorlevel 1 (
    echo   [未找到] >> "%REPORT%"
)
echo. >> "%REPORT%"

echo --- HKCU (当前用户) --- >> "%REPORT%"
reg query "HKCU\SOFTWARE\Microsoft\EdgeUpdate\Clients\%REG_GUID%" /v pv >> "%REPORT%" 2>nul
if errorlevel 1 (
    echo   [未找到] >> "%REPORT%"
)
echo. >> "%REPORT%"

echo --- HKLM 安装路径 --- >> "%REPORT%"
reg query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\%REG_GUID%" /v pv >> "%REPORT%" 2>nul
reg query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\%REG_GUID%" /v ap >> "%REPORT%" 2>nul
echo. >> "%REPORT%"

echo --- HKCU 安装路径 --- >> "%REPORT%"
reg query "HKCU\SOFTWARE\Microsoft\EdgeUpdate\Clients\%REG_GUID%" /v pv >> "%REPORT%" 2>nul
reg query "HKCU\SOFTWARE\Microsoft\EdgeUpdate\Clients\%REG_GUID%" /v ap >> "%REPORT%" 2>nul
echo. >> "%REPORT%"

:: ========== 5. WebView2 进程检查 ==========
echo [5/7] 正在检查 WebView2 进程...
echo [5/7] WebView2 相关进程 >> "%REPORT%"
echo -------------------------------------------- >> "%REPORT%"
tasklist /FI "IMAGENAME eq msedgewebview2.exe" /FO CSV /NH >> "%REPORT%" 2>nul
if errorlevel 1 (
    echo   无 WebView2 进程运行 >> "%REPORT%"
)
echo. >> "%REPORT%"

:: ========== 6. Edge 浏览器检查 ==========
echo [6/7] 正在检查 Edge 浏览器...
echo [6/7] Microsoft Edge 浏览器检查 >> "%REPORT%"
echo -------------------------------------------- >> "%REPORT%"
echo --- Edge 版本 (注册表) --- >> "%REPORT%"
reg query "HKLM\SOFTWARE\Microsoft\Edge\BLBeacon" /v version >> "%REPORT%" 2>nul
if errorlevel 1 (
    reg query "HKCU\SOFTWARE\Microsoft\Edge\BLBeacon" /v version >> "%REPORT%" 2>nul
)
if errorlevel 1 (
    echo   [未找到 Edge 版本信息] >> "%REPORT%"
)
echo. >> "%REPORT%"
echo --- Edge 安装路径 (注册表) --- >> "%REPORT%"
reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe" /ve >> "%REPORT%" 2>nul
if errorlevel 1 (
    echo   [未找到 Edge 安装路径] >> "%REPORT%"
)
echo. >> "%REPORT%"

:: ========== 7. C盘管家安装信息 ==========
echo [7/7] 正在检查 C盘管家安装信息...
echo [7/7] C盘管家安装信息 >> "%REPORT%"
echo -------------------------------------------- >> "%REPORT%"
echo --- 安装路径 (注册表) --- >> "%REPORT%"
reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall" /s /f "C盘管家" >> "%REPORT%" 2>nul
if errorlevel 1 (
    reg query "HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall" /s /f "C盘管家" >> "%REPORT%" 2>nul
)
if errorlevel 1 (
    echo   [注册表中未找到 C盘管家] >> "%REPORT%"
)
echo. >> "%REPORT%"

:: ========== 8. 结论判定（反馈 P 修正后的判据） ==========
echo [8/8] 正在生成结论...
echo [8/8] 结论 >> "%REPORT%"
echo -------------------------------------------- >> "%REPORT%"
if "%WV2_PV%"=="" (
    echo   注册表无 pv：WebView2 未安装，C盘管家安装包会自动引导安装。 >> "%REPORT%"
) else if "%WV2_X64_FOUND%"=="0" (
    echo   [异常] 注册表 pv=%WV2_PV% 但缺 x64 实体组件！ >> "%REPORT%"
    echo   这正是"装完 C盘管家打不开"的根因（只有 x86 老版 Runtime）。 >> "%REPORT%"
    echo   修复：安装微软官方 WebView2 Evergreen 引导程序（自动补齐 64 位组件）： >> "%REPORT%"
    echo   https://developer.microsoft.com/zh-cn/microsoft-edge/webview2/ >> "%REPORT%"
) else (
    echo   [正常] pv=%WV2_PV% 且找到 x64 实体组件，WebView2 环境可用。 >> "%REPORT%"
)
echo. >> "%REPORT%"

echo ============================================ >> "%REPORT%"
echo   诊断完成！                                 >> "%REPORT%"
echo   报告已保存到桌面:                           >> "%REPORT%"
echo   %REPORT%                                   >> "%REPORT%"
echo ============================================ >> "%REPORT%"

echo.
echo 诊断完成！报告已生成到桌面：
echo %REPORT%
echo.
echo 请将此文件发送给 C盘管家开发者进行分析。
echo.
pause
