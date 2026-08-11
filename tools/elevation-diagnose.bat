@echo off
chcp 65001 >nul 2>&1
title C盘管家 - 提权环境诊断

:: 启动器：实际诊断逻辑在同目录的 elevation-diagnose.ps1（纯 PowerShell，避免 cmd 解析中文多行代码的编码坑）
:: 故意不以管理员身份运行，以复现普通用户遇到的提权场景

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0elevation-diagnose.ps1"

echo.
pause
