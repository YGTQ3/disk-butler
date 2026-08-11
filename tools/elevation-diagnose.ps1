# C盘管家 - 提权环境诊断（纯 PowerShell 版）
# 注意：此脚本故意不以管理员身份运行，以复现普通用户遇到的提权场景。
# 只读收集，不修改系统、不删文件、不联网。

$ErrorActionPreference = 'Continue'

# ========== 透明告知 ==========
Write-Host '============================================'
Write-Host '  C盘管家 提权环境诊断'
Write-Host '============================================'
Write-Host ''
Write-Host '这个脚本会做什么（透明告知）：'
Write-Host '  - 只收集 8 项系统信息：系统版本、用户权限、UAC 配置、'
Write-Host '    安全策略、已装安全软件、应用控制策略、提权测试、事件日志'
Write-Host '  - 不修改任何系统设置，不删除任何文件，不联网'
Write-Host '  - 不收集您的个人文件、账号、聊天记录'
Write-Host '  - 测试提权时会弹出 UAC 授权框，点"是"或"否"都可以，'
Write-Host '    结果都会如实记录（这正是要测的）'
Write-Host '  - 报告只保存在您桌面，是否发送完全由您决定'
Write-Host ''
try { $null = Read-Host '按回车键开始，或关闭此窗口取消' } catch {}

# ========== 报告路径（兼容 OneDrive 重定向） ==========
$desktop = [Environment]::GetFolderPath('Desktop')
if (-not $desktop) { $desktop = Join-Path $env:USERPROFILE 'Desktop' }
$REPORT = Join-Path $desktop 'disk-butler-elevation-report.txt'

function Log([string]$s) { Add-Content -LiteralPath $REPORT -Value $s -Encoding UTF8 }

Set-Content -LiteralPath $REPORT -Encoding UTF8 -Value @(
    '============================================',
    '  C盘管家 提权环境诊断报告',
    ('  生成时间: ' + (Get-Date).ToString('yyyy-MM-dd HH:mm:ss')),
    '  脚本权限: 普通用户（非提权）',
    '============================================',
    ''
)

# ========== [1/8] 系统信息 ==========
Write-Host '[1/8] 正在收集系统信息...'
Log '[1/8] 系统信息'
Log '--------------------------------------------'
$si = systeminfo
$matched = $si | Select-String 'OS 名称|OS 版本|系统类型|处理器|BIOS'
if (-not $matched) {
    $matched = $si | Select-String 'OS Name|OS Version|System Type|Processor|BIOS'
}
$matched | ForEach-Object { Log $_.Line.Trim() }
Log ''

# ========== [2/8] 用户权限 ==========
Write-Host '[2/8] 正在检查用户权限...'
Log '[2/8] 用户权限信息'
Log '--------------------------------------------'
Log ('用户名: ' + $env:USERNAME)
Log ('用户目录: ' + $env:USERPROFILE)
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
Log ('当前令牌是否管理员: ' + $isAdmin)
Log ''
Log '--- 用户所属组（管理员组 SID S-1-5-32-544，不受中文本地化影响） ---'
$admGroup = (whoami /groups /fo csv) | Select-String 'S-1-5-32-544'
if ($admGroup) { $admGroup | ForEach-Object { Log $_.Line } } else { Log '  [未找到管理员组 SID S-1-5-32-544]' }
Log ''

# ========== [3/8] UAC 配置 ==========
Write-Host '[3/8] 正在检查 UAC 设置...'
Log '[3/8] UAC 配置'
Log '--------------------------------------------'
$polKey = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
$uacNames = @('EnableLUA','ConsentPromptBehaviorAdmin','ConsentPromptBehaviorUser','PromptOnSecureDesktop','FilterAdministratorToken','LocalAccountTokenFilterPolicy')
foreach ($n in $uacNames) {
    $val = (Get-ItemProperty $polKey -Name $n -EA SilentlyContinue).$n
    if ($null -ne $val) { Log ($n + ': ' + $val) } else { Log ($n + ': [未设置]') }
}
Log ''
Log '--- UAC 等级解读 ---'
$v = (Get-ItemProperty $polKey -Name ConsentPromptBehaviorAdmin -EA SilentlyContinue).ConsentPromptBehaviorAdmin
$level = switch ([string]$v) {
    '0' { 'UAC等级: 从不通知（已关闭）' }
    '1' { 'UAC等级: 仅在程序尝试更改时通知（不提示桌面变暗）' }
    '2' { 'UAC等级: 仅在程序尝试更改时通知（默认）' }
    '3' { 'UAC等级: 仅在非 Windows 程序尝试更改时通知' }
    '4' { 'UAC等级: 仅在非 Windows 程序尝试更改时通知（不提示桌面变暗）' }
    '5' { 'UAC等级: 仅在程序尝试更改计算机时通知' }
    default { 'UAC等级: 未知值(' + $v + ')' }
}
Log $level
Log ''

# ========== [4/8] 本地安全策略 ==========
Write-Host '[4/8] 正在检查安全策略...'
Log '[4/8] 本地安全策略（与提权相关）'
Log '--------------------------------------------'
$cfg = Join-Path $env:TEMP 'diskbutler-secpolicy.cfg'
secedit /export /cfg $cfg /areas USER_RIGHTS *> $null
if (Test-Path $cfg) {
    Log '--- 用户权限分配（提权相关项） ---'
    $lines = Get-Content $cfg
    $lines | Select-String 'SeDeny' | ForEach-Object { Log $_.Line.Trim() }
    $lines | Select-String 'SeDebugPrivilege|SeTakeOwnershipPrivilege|SeBackupPrivilege|SeRestorePrivilege' | ForEach-Object { Log $_.Line.Trim() }
    Remove-Item $cfg -Force -EA SilentlyContinue
} else {
    Log '  [无法导出安全策略，可能需要管理员权限]'
}
Log ''

# ========== [5/8] 第三方安全软件 + Defender ==========
Write-Host '[5/8] 正在检查第三方安全软件...'
Log '[5/8] 已安装的安全/杀毒软件'
Log '--------------------------------------------'
$av_keywords = @('360','火绒','Huorong','电脑管家','TencentPCMgr','安全卫士','Kaspersky','卡巴斯基','Norton','诺顿','McAfee','迈克菲','ESET','Bitdefender','Malwarebytes','Avast','AVG','Sophos','Trend Micro','趋势','Symantec','赛门铁克','Windows Defender','Microsoft Defender','Security Health','安全中心','火绒安全','奇安信','天擎','深信服','安天','江民','金山毒霸','Kingsoft','Dr.Web','大蜘蛛','小红伞','Avira','Comodo','Panda','F-Secure','G Data','Emsisoft','Webroot','Cylance','CrowdStrike','SentinelOne')
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
                if ($sw.DisplayName -like "*$kw*") {
                    $found += ($sw.DisplayName.ToString() + ' (' + $sw.DisplayVersion + ')')
                    break
                }
            }
        }
    }
}
$found = $found | Select-Object -Unique
if ($found.Count -gt 0) { $found | ForEach-Object { Log $_ } } else { Log '[未检测到常见安全软件]' }
Log ''
Log '--- Windows Defender 状态 ---'
try {
    $mp = Get-MpPreference -EA SilentlyContinue
    if ($mp) {
        Log ('实时防护: ' + $(if ($mp.DisableRealtimeMonitoring) { '已关闭' } else { '已开启' }))
        Log ('行为监控: ' + $(if ($mp.DisableBehaviorMonitoring) { '已关闭' } else { '已开启' }))
        Log ('篡改防护: ' + $(if ($mp.DisableTamperProtection) { '已关闭' } else { '已开启' }))
    } else {
        $tp = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows Defender\Features' -Name TamperProtection -EA SilentlyContinue).TamperProtection
        if ($tp -eq 1) { Log '篡改防护: 已开启（通过注册表确认）' }
        elseif ($tp -eq 0) { Log '篡改防护: 已关闭（通过注册表确认）' }
        else { Log '[无法获取 Defender 偏好设置，且注册表也无相关项]' }
    }
} catch {
    Log ('[获取 Defender 状态失败: ' + $_.Exception.Message + ']')
}
Log ''

# ========== [6/8] AppLocker / WDAC ==========
Write-Host '[6/8] 正在检查 AppLocker / 应用控制策略...'
Log '[6/8] 应用控制策略'
Log '--------------------------------------------'
if (Get-Command Get-AppLockerPolicy -EA SilentlyContinue) {
    try {
        $al = Get-AppLockerPolicy -Effective -EA SilentlyContinue
        if ($al) {
            Log ('AppLocker 规则集合数: ' + $al.RuleCollections.Count)
            foreach ($rc in $al.RuleCollections) {
                Log ('  类型: ' + $rc.CollectionType + ' 模式: ' + $rc.EnforcementMode + ' 规则数: ' + $rc.Rules.Count)
            }
        } else { Log '[AppLocker 未配置]' }
    } catch {
        Log ('[AppLocker 查询失败: ' + $_.Exception.Message + ']')
    }
} else {
    Log '[AppLocker 不可用（Windows 家庭版不带此功能，可排除此项）]'
}
$wdac = Get-CimInstance -ClassName Win32_DeviceGuard -Namespace 'root\Microsoft\Windows\DeviceGuard' -EA SilentlyContinue
if ($wdac) {
    Log ('WDAC 代码完整性策略: ' + $wdac.CodeIntegrityPolicyEnforcementStatus)
    Log ('WDAC 用户模式代码策略: ' + $wdac.UsermodeCodeIntegrityPolicyEnforcementStatus)
} else { Log '[WDAC 信息不可用]' }
Log ''

# ========== [7/8] 提权通道测试 ==========
Write-Host '[7/8] 正在模拟提权测试（可能弹出 UAC 授权框，点是或否均可）...'
Log '[7/8] 提权通道测试'
Log '--------------------------------------------'

function Test-Elevation([string]$target, [string[]]$argsList, [string]$label) {
    Log ('--- ' + $label + ' ---')
    try {
        $start = Get-Date
        $job = Start-Job -ScriptBlock {
            param($startTime, $tgt, $ag)
            try {
                $p = Start-Process -Verb RunAs -Wait -PassThru -FilePath $tgt -ArgumentList $ag -ErrorAction Stop
                return @{ Success = $true; ExitCode = $p.ExitCode; Elapsed = ((Get-Date) - $startTime).TotalSeconds }
            } catch [System.ComponentModel.Win32Exception] {
                return @{ Success = $false; NativeErrorCode = $_.Exception.NativeErrorCode; Message = $_.Exception.Message }
            } catch {
                return @{ Success = $false; ExceptionType = $_.Exception.GetType().Name; Message = $_.Exception.Message }
            }
        } -ArgumentList @($start, $target, $argsList)
        $completed = Wait-Job $job -Timeout 30
        if ($completed) {
            $result = Receive-Job $job
            if ($result.Success) {
                Log ('结果: 成功（耗时 ' + [math]::Round($result.Elapsed, 1) + ' 秒，退出码 ' + $result.ExitCode + '）')
            } else {
                Log '结果: 被系统拦截'
                if ($result.NativeErrorCode) { Log ('错误码: 0x' + ('{0:X8}' -f [int]$result.NativeErrorCode)) }
                Log ('错误信息: ' + $result.Message)
            }
        } else {
            Log '结果: 超时（30秒内未完成，可能 UAC 弹窗等待用户操作或被策略阻止）'
            Stop-Job $job
        }
        Remove-Job $job -Force -EA SilentlyContinue
    } catch {
        Log '结果: 脚本执行异常'
        Log ('错误: ' + $_.Exception.Message)
    }
    Log ''
}

Test-Elevation -target 'powershell.exe' -argsList @('-NoProfile','-Command','Write-Output OK') -label '测试 A: PowerShell RunAs 提权'
Test-Elevation -target 'cmd.exe' -argsList @('/d','/c','echo OK') -label '测试 B: cmd.exe RunAs 提权（模拟 C盘管家调用方式）'

Log '--- 测试 C: 当前进程令牌信息 ---'
try {
    $id = [System.Security.Principal.WindowsIdentity]::GetCurrent()
    Log ('认证类型: ' + $id.AuthenticationType)
    Log ('是否管理员: ' + (New-Object System.Security.Principal.WindowsPrincipal($id)).IsInRole([System.Security.Principal.WindowsBuiltInRole]::Administrator))
} catch {
    Log ('[获取令牌信息失败: ' + $_.Exception.Message + ']')
}
(whoami /user /fo csv) | ForEach-Object { Log $_ }
Log ''
$privs = (whoami /priv /fo csv) | Select-String 'SeDebugPrivilege|SeTakeOwnershipPrivilege|SeBackupPrivilege|SeRestorePrivilege|SeImpersonatePrivilege'
if ($privs) { $privs | ForEach-Object { Log $_.Line } } else { Log '[当前令牌无特殊特权]' }
Log ''

# ========== [8/8] 事件日志 ==========
Write-Host '[8/8] 正在检查事件日志...'
Log '[8/8] 相关事件日志（最近 24 小时）'
Log '--------------------------------------------'
try {
    $since = (Get-Date).AddHours(-24)
    $events = @()
    $appEvents = Get-WinEvent -FilterHashtable @{ LogName = 'Application'; Level = 2; StartTime = $since } -MaxEvents 20 -EA SilentlyContinue
    if ($appEvents) {
        $appEvents | Where-Object { $_.Message -match 'UAC|elevat|RunAs|Shell|权限|提权|拒绝|denied|access' } | ForEach-Object {
            $events += ('[App] ' + $_.TimeCreated.ToString('yyyy-MM-dd HH:mm:ss') + ' | ' + $_.ProviderName + ' | EventID=' + $_.Id + ' | ' + $_.Message.Substring(0, [Math]::Min(200, $_.Message.Length)))
        }
    }
    try {
        $secEvents = Get-WinEvent -FilterHashtable @{ LogName = 'Security'; ID = 4688,4689,4673,4674; StartTime = $since } -MaxEvents 10 -EA SilentlyContinue
        if ($secEvents) {
            $secEvents | ForEach-Object {
                $events += ('[Sec] ' + $_.TimeCreated.ToString('yyyy-MM-dd HH:mm:ss') + ' | EventID=' + $_.Id + ' | ' + $_.Message.Substring(0, [Math]::Min(200, $_.Message.Length)))
            }
        }
    } catch {
        $events += '[Security 日志需要管理员权限才能读取，跳过]'
    }
    if ($events.Count -gt 0) { $events | ForEach-Object { Log $_ } } else { Log '[最近 24 小时未找到相关错误事件]' }
} catch {
    Log ('[事件日志查询失败: ' + $_.Exception.Message + ']')
}
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
Write-Host '主题请写：C盘管家·提权诊断报告'
Write-Host ''
Write-Host '如果不想发送，直接关闭即可，不影响使用。'

