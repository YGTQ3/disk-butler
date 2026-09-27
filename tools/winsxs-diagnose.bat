@echo off
chcp 65001 >nul 2>&1
title C盘管家 - WinSxS/DISM 诊断

:: 启动器：实际诊断逻辑在同目录的 winsxs-diagnose.ps1（纯 PowerShell，避免 cmd 解析中文多行代码的编码坑）
:: 需要管理员权限：读取 DISM/CBS 日志、采样系统进程、运行只读 DISM 分析。未提权则自提权重启。
:: 全程只读收集：不修改系统、不删除文件、不联网、不收集个人文件。

net session >nul 2>&1
if %errorlevel% neq 0 (
    echo 本工具需要管理员权限，正在请求授权，请在弹出的 UAC 窗口点"是"...
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0winsxs-diagnose.ps1"

echo.
pause
