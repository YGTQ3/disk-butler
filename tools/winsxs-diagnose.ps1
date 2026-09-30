# C盘管家 - WinSxS / DISM 深度清理诊断（纯 PowerShell 版）
# 用途：当"清理 Windows 更新旧版本备份 (WinSxS)"分析卡住 / 超时 / 报错时，一键采集定位证据。
# 只读收集：不修改系统、不删除文件、不联网、不收集个人文件（用户名/文档/账号/聊天记录一概不碰）。
# 需要管理员权限（读取 DISM/CBS 日志、采样系统进程、运行只读 DISM 分析）——启动器 .bat 已自提权。
# 若直接以普通权限运行本 .ps1，会尝试自提权重启一次。

$ErrorActionPreference = 'Continue'

# ========== 若非管理员则自提权重启（.bat 已提权时此处直接跳过） ==========
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host '本工具需要管理员权限（读取系统日志、运行只读 DISM 分析），正在请求授权...'
    try {
        Start-Process -FilePath 'powershell.exe' -Verb RunAs -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"")
    } catch {
        Write-Host '未获得管理员权限，部分系统日志可能读取失败。'
    }
    return
}

# ========== 透明告知 ==========
Write-Host '============================================'
Write-Host '  C盘管家 WinSxS / DISM 诊断'
Write-Host '============================================'
Write-Host ''
Write-Host '这个脚本会做什么（透明告知）：'
Write-Host '  - 只读收集 8 项信息：系统与磁盘、挂起更新状态、关键服务、'
Write-Host '    安全软件、DISM 只读分析交叉验证、DISM 日志、CBS 日志、系统事件日志'
Write-Host '  - 会运行一次微软官方只读分析命令 Dism /AnalyzeComponentStore'
Write-Host '    （与 C盘管家用的是同一条命令，只分析、不更改系统）'
Write-Host '  - 该分析带 7 分钟硬超时：正常 1~3 分钟出结果；若超时说明它确实卡住了，'
Write-Host '    脚本会自动结束这次只读分析（只读操作，中断安全）并记录卡在哪'
Write-Host '  - 不修改任何系统设置，不删除任何文件，不联网'
Write-Host '  - 不收集您的个人文件、账号、聊天记录'
Write-Host '  - 报告只保存在您桌面，是否发送完全由您决定'
Write-Host ''
try { $null = Read-Host '按回车键开始，或关闭此窗口取消' } catch {}

# ========== 报告路径（兼容 OneDrive 重定向） ==========
$desktop = [Environment]::GetFolderPath('Desktop')
if (-not $desktop) { $desktop = Join-Path $env:USERPROFILE 'Desktop' }
$REPORT = Join-Path $desktop 'disk-butler-winsxs-report.txt'

function Log([string]$s) { Add-Content -LiteralPath $REPORT -Value $s -Encoding UTF8 }

Set-Content -LiteralPath $REPORT -Encoding UTF8 -Value @(
    '============================================',
    '  C盘管家 WinSxS / DISM 诊断报告',
    ('  生成时间: ' + (Get-Date).ToString('yyyy-MM-dd HH:mm:ss')),
    '  脚本权限: 管理员（已提权）',
    '  说明: 全程只读，未修改/删除任何文件，未联网',
    '============================================',
    ''
)

# ========== [1/8] 系统与磁盘信息 ==========
Write-Host '[1/8] 正在收集系统与磁盘信息...'
Log '[1/8] 系统与磁盘信息'
Log '--------------------------------------------'
try {
    $osKey = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -ErrorAction Stop
    Log ('OS 名称: ' + $osKey.ProductName)
    $build = $osKey.CurrentBuild
    if ($osKey.UBR) { $build = "$build." + $osKey.UBR }
    if ($osKey.DisplayVersion) { $build += ' (' + $osKey.DisplayVersion + ')' }
    Log ('OS 版本: ' + $build)
} catch { Log '[注册表读取 OS 信息失败]' }
try {
    $cs = Get-CimInstance Win32_ComputerSystem
    Log ('内存: ' + [math]::Round($cs.TotalPhysicalMemory / 1GB, 1) + ' GB')
    $cpu = Get-CimInstance Win32_Processor
    Log ('处理器: ' + $cpu.Name)
} catch { Log ('[硬件信息读取失败: ' + $_.Exception.Message + ']') }
try {
    $d = Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='C:'"
    Log ('C盘 总容量: ' + [math]::Round($d.Size / 1GB, 1) + ' GB   剩余: ' + [math]::Round($d.FreeSpace / 1GB, 1) + ' GB')
} catch { Log ('[C盘空间读取失败: ' + $_.Exception.Message + ']') }
Log '--- 物理磁盘（类型 SSD/HDD 与健康，慢盘/坏盘会拖慢 DISM 分析） ---'
try {
    Get-PhysicalDisk | ForEach-Object {
        Log ('  磁盘' + $_.DeviceId + ': ' + $_.FriendlyName + '  类型=' + $_.MediaType + '  总线=' + $_.BusType + '  容量=' + [math]::Round($_.Size / 1GB, 0) + 'GB  健康=' + $_.HealthStatus)
    }
} catch { Log ('  [物理磁盘信息不可用: ' + $_.Exception.Message + ']') }
Log ''

# ========== [2/8] 挂起更新 / 待重启状态 ==========
Write-Host '[2/8] 正在检查挂起更新 / 待重启状态...'
Log '[2/8] 挂起更新 / 待重启状态（DISM 卡死最常见根因之一）'
Log '--------------------------------------------'
$cbsPending = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending'
$wuReboot = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired'
Log ('CBS RebootPending 键存在: ' + (Test-Path $cbsPending))
Log ('WindowsUpdate RebootRequired 键存在: ' + (Test-Path $wuReboot))
$pendingXml = 'C:\Windows\WinSxS\pending.xml'
if (Test-Path $pendingXml) {
    Log ('WinSxS\pending.xml 存在: True   修改时间: ' + (Get-Item $pendingXml).LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss'))
} else { Log 'WinSxS\pending.xml 存在: False' }
Log ('poqexec.log 存在: ' + (Test-Path 'C:\Windows\poqexec.log'))
$tw = Get-Process TiWorker -EA SilentlyContinue
Log ('当前是否有 TiWorker 在运行: ' + [bool]$tw)
Log '--- 最近安装的更新（Get-HotFix 前 5） ---'
try {
    Get-HotFix | Sort-Object InstalledOn -Descending | Select-Object -First 5 | ForEach-Object {
        Log ('  ' + $_.HotFixID + '   ' + $_.InstalledOn)
    }
} catch { Log ('  [Get-HotFix 失败: ' + $_.Exception.Message + ']') }
Log ''

# ========== [3/8] 关键服务状态 ==========
Write-Host '[3/8] 正在检查关键服务状态...'
Log '[3/8] 关键服务状态（DISM 依赖 TrustedInstaller，被禁用会失败/卡住）'
Log '--------------------------------------------'
$svcMap = [ordered]@{
    'TrustedInstaller' = 'Windows Modules Installer（组件安装器，DISM 核心依赖）'
    'msiserver'        = 'Windows Installer'
    'wuauserv'         = 'Windows Update'
    'DoSvc'            = 'Delivery Optimization（传递优化）'
    'BITS'             = '后台智能传输'
}
foreach ($k in $svcMap.Keys) {
    try {
        $s = Get-CimInstance Win32_Service -Filter "Name='$k'" -EA Stop
        Log ($k + ' (' + $svcMap[$k] + '): 状态=' + $s.State + '  启动类型=' + $s.StartMode)
    } catch { Log ($k + ': [未找到该服务或读取失败]') }
}
Log ''

# ========== [4/8] 第三方安全软件 + Defender ==========
Write-Host '[4/8] 正在检查安全软件...'
Log '[4/8] 已安装的安全/杀毒软件（安全软件拦截系统文件访问会拖慢/阻断 DISM）'
Log '--------------------------------------------'
$av_keywords = @('360', '火绒', 'Huorong', '电脑管家', 'TencentPCMgr', '安全卫士', 'Kaspersky', '卡巴斯基', 'Norton', '诺顿', 'McAfee', '迈克菲', 'ESET', 'Bitdefender', 'Malwarebytes', 'Avast', 'AVG', 'Sophos', 'Trend Micro', '趋势', 'Symantec', '赛门铁克', '火绒安全', '奇安信', '天擎', '深信服', '安天', '江民', '金山毒霸', 'Kingsoft', 'Dr.Web', '大蜘蛛', '小红伞', 'Avira', 'Comodo', 'Panda', 'F-Secure', 'G Data', 'Emsisoft', 'Webroot', 'CrowdStrike', 'SentinelOne')
$paths = @(
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
)
$found = @()
foreach ($p in $paths) {
    Get-ItemProperty $p -EA SilentlyContinue | ForEach-Object {
        $sw = $_
        if ($sw.DisplayName) {
            foreach ($kw in $av_keywords) {
                if ($sw.DisplayName -like "*$kw*") { $found += ($sw.DisplayName.ToString() + ' (' + $sw.DisplayVersion + ')'); break }
            }
        }
    }
}
$found = $found | Select-Object -Unique
if ($found.Count -gt 0) { $found | ForEach-Object { Log ('  ' + $_) } } else { Log '  [未检测到常见第三方安全软件]' }
try {
    $mp = Get-MpPreference -EA SilentlyContinue
    if ($mp) { Log ('Windows Defender 实时防护: ' + $(if ($mp.DisableRealtimeMonitoring) { '已关闭' } else { '已开启' })) }
} catch { Log '[Defender 偏好读取失败，跳过]' }
Log ''

# ========== [5/8] DISM 只读分析交叉验证（带硬超时 + 活体采样）核心 ==========
Write-Host '[5/8] 正在运行 DISM 只读分析交叉验证（正常 1~3 分钟，最多等 7 分钟）...'
Write-Host '       若弹出 UAC 请点"是"；此步只分析、不更改系统。'
Log '[5/8] DISM 只读分析交叉验证（核心）'
Log '--------------------------------------------'
Log '命令: Dism /Online /Cleanup-Image /AnalyzeComponentStore  （与 C盘管家同款，只读）'
$maxSec = 420
Log ('硬超时: ' + $maxSec + ' 秒（超过即判定为异常卡住，自动结束本次只读分析）')
$dismOut = Join-Path $env:TEMP ('diskbutler-winsxs-analyze-' + [guid]::NewGuid().ToString('N') + '.txt')

$job = Start-Job -ScriptBlock {
    param($out)
    $ErrorActionPreference = 'Continue'
    & Dism.exe /Online /Cleanup-Image /AnalyzeComponentStore 2>&1 | Out-File -LiteralPath $out -Encoding UTF8
    return $LASTEXITCODE
} -ArgumentList $dismOut

$sw = [Diagnostics.Stopwatch]::StartNew()
$prev = @{}
$done = $false
Log '--- 等待期间活体采样（TiWorker/Dism 是否在真干活：区间 CPU/IO 为 0 = 卡住等待，非 0 = 在跑只是慢） ---'
while ($sw.Elapsed.TotalSeconds -lt $maxSec) {
    if (Wait-Job $job -Timeout 10) { $done = $true; break }
    $now = [int]$sw.Elapsed.TotalSeconds
    $ps = Get-CimInstance Win32_Process -Filter "Name='TiWorker.exe' OR Name='Dism.exe' OR Name='TrustedInstaller.exe'" -EA SilentlyContinue
    if ($ps) {
        foreach ($p in $ps) {
            $cpu = ($p.UserModeTime + $p.KernelModeTime) / 1e7
            $io = ($p.ReadTransferCount + $p.WriteTransferCount) / 1MB
            $key = [string]$p.ProcessId
            $extra = ''
            if ($prev.ContainsKey($key)) {
                $extra = '  区间CPU=+' + [math]::Round($cpu - $prev[$key].cpu, 1) + 's  区间IO=+' + [math]::Round($io - $prev[$key].io, 1) + 'MB'
            }
            Log ('  t=' + $now + 's  ' + $p.Name + ' PID=' + $p.ProcessId + '  累计CPU=' + [math]::Round($cpu, 1) + 's  累计IO=' + [math]::Round($io, 1) + 'MB' + $extra)
            $prev[$key] = @{ cpu = $cpu; io = $io }
        }
    } else {
        Log ('  t=' + $now + 's  [尚未发现 Dism/TiWorker 进程]')
    }
}

if ($done) {
    $code = Receive-Job $job
    Log ''
    Log ('结果: 分析在 ' + [math]::Round($sw.Elapsed.TotalSeconds, 1) + ' 秒内完成，退出码 ' + $code)
    Log '--- DISM 输出原文 ---'
    try {
        $text = Get-Content -LiteralPath $dismOut -Raw -EA Stop
        ($text -split "`r?`n") | Where-Object { $_.Trim() -ne '' -and -not $_.Trim().StartsWith('[') } | ForEach-Object { Log ('  ' + $_.Trim()) }
    } catch { Log '  [读取 DISM 输出失败]' }
} else {
    Log ''
    Log ('结果: 超时（' + $maxSec + ' 秒内未完成）—— 判定为异常卡住，与用户反馈现象一致')
    Log '说明: 上方采样若显示 TiWorker/Dism 区间 CPU/IO 长期为 0，则为死锁/等待（多为挂起更新、组件存储损坏或安全软件拦截）；若持续非 0，则为真在跑但极慢（多为磁盘慢/组件存储巨大）。'
    # 结束本次只读分析，避免持续占用导致卡顿（AnalyzeComponentStore 只读，中断安全）
    Stop-Job $job -EA SilentlyContinue
    Get-Process Dism -EA SilentlyContinue | Stop-Process -Force -EA SilentlyContinue
    Get-CimInstance Win32_Process -Filter "Name='TiWorker.exe'" -EA SilentlyContinue | ForEach-Object { try { Stop-Process -Id $_.ProcessId -Force -EA SilentlyContinue } catch {} }
    Log '已自动结束本次只读分析进程（只读操作，中断不影响系统）。'
}
Remove-Job $job -Force -EA SilentlyContinue
Remove-Item -LiteralPath $dismOut -Force -EA SilentlyContinue
Log ''

# ========== [6/8] DISM 日志尾部 ==========
Write-Host '[6/8] 正在读取 DISM 日志尾部...'
Log '[6/8] DISM 日志尾部（C:\Windows\Logs\DISM\dism.log 最后 80 行，最权威的卡点证据）'
Log '--------------------------------------------'
$dismLog = 'C:\Windows\Logs\DISM\dism.log'
if (Test-Path $dismLog) {
    Log ('  日志最后修改时间: ' + (Get-Item $dismLog).LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss'))
    try { Get-Content -LiteralPath $dismLog -Tail 80 -EA Stop | ForEach-Object { Log ('  ' + $_) } }
    catch { Log ('  [读取 dism.log 失败: ' + $_.Exception.Message + ']') }
} else { Log '  [dism.log 不存在]' }
Log ''

# ========== [7/8] CBS 日志尾部 ==========
Write-Host '[7/8] 正在读取 CBS 日志尾部...'
Log '[7/8] CBS 日志尾部（C:\Windows\Logs\CBS\CBS.log 最后 60 行，组件存储/servicing 细节）'
Log '--------------------------------------------'
$cbsLog = 'C:\Windows\Logs\CBS\CBS.log'
if (Test-Path $cbsLog) {
    Log ('  日志最后修改时间: ' + (Get-Item $cbsLog).LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss'))
    try { Get-Content -LiteralPath $cbsLog -Tail 60 -EA Stop | ForEach-Object { Log ('  ' + $_) } }
    catch { Log ('  [读取 CBS.log 失败: ' + $_.Exception.Message + ']') }
} else { Log '  [CBS.log 不存在]' }
Log ''

# ========== [8/8] 系统事件日志 ==========
Write-Host '[8/8] 正在检查系统事件日志（最近 7 天）...'
Log '[8/8] 系统事件日志（最近 7 天，Servicing / WindowsUpdate / DISM 相关错误与警告）'
Log '--------------------------------------------'
try {
    $since = (Get-Date).AddDays(-7)
    $ev = Get-WinEvent -FilterHashtable @{ LogName = 'System'; Level = 2, 3; StartTime = $since } -MaxEvents 200 -EA SilentlyContinue
    $hits = $ev | Where-Object { $_.ProviderName -match 'Servicing|WindowsUpdate|DISM|TrustedInstaller|Application Popup|Winlogon' } | Select-Object -First 25
    if ($hits) {
        $hits | ForEach-Object {
            $msg = $_.Message
            if ($msg.Length -gt 220) { $msg = $msg.Substring(0, 220) }
            Log ('  ' + $_.TimeCreated.ToString('yyyy-MM-dd HH:mm:ss') + ' | ' + $_.ProviderName + ' | ID=' + $_.Id + ' | ' + ($msg -replace "`r?`n", ' '))
        }
    } else { Log '  [最近 7 天未找到相关错误/警告事件]' }
} catch { Log ('  [事件日志查询失败: ' + $_.Exception.Message + ']') }
Log ''

# ========== 结论 ==========
Log '============================================'
Log '  诊断完成！'
Log ('  报告已保存到桌面: ' + $REPORT)
Log '============================================'

Write-Host ''
Write-Host '诊断完成！报告已生成到桌面：'
Write-Host $REPORT
Write-Host ''
Write-Host '如果您愿意帮助排查问题，可以将此文件发送给：'
Write-Host 'ygtq1021@126.com'
Write-Host '主题请写：C盘管家·WinSxS/DISM 诊断报告'
Write-Host ''
Write-Host '发送前建议用记事本打开自己先看一遍。如果不想发送，直接关闭即可，不影响使用。'
