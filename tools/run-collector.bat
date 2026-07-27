@echo off
rem DiskButler 规则采集器启动器（双击运行）；本文件为 GBK 编码，请勿以 UTF-8 保存
rem 需与 collect-rules.ps1 放在同一文件夹
cd /d "%~dp0"
if not exist "collect-rules.ps1" (
    echo [错误] 本文件夹里没有找到 collect-rules.ps1
    echo 请从这里下载：https://github.com/YGTQ3/disk-butler/tree/main/tools
    pause
    exit /b 1
)
echo ==============================================
echo   C盘管家 - 规则采集器
echo   不删除、不修改任何东西，也不联网。
echo   报告会生成在你的桌面上。
echo ==============================================
echo.
echo   [1] 基础模式（推荐）：扫描软件缓存区，约 1 分钟
echo   [2] 完整模式：另外收集各磁盘的大目录线索，约 5~10 分钟
echo       （注意：报告会包含 D 盘等顶层文件夹的名字）
echo.
set MODE=
set /p MODE=请输入 1 或 2 后按回车（直接回车 = 基础模式）：
echo.
if "%MODE%"=="2" (
    echo 已选择：完整模式
    powershell -NoProfile -ExecutionPolicy Bypass -File "collect-rules.ps1" -IncludeDrives
) else (
    echo 已选择：基础模式
    powershell -NoProfile -ExecutionPolicy Bypass -File "collect-rules.ps1"
)
echo.
echo 完成！报告文件在你的桌面上（diskbutler-rule-report-*.md）。
echo 请先用记事本打开看一遍，确认没问题再分享。
pause
