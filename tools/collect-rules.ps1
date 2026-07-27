# =====================================================================
# DiskButler Rule Collector v1.0  (ASCII only, PS 5.1 compatible)
# Purpose: scan THIS machine for cleanup-rule candidates and produce a
#          privacy-safe, human-readable report you can review and share.
# Collects: top-level dir names + sizes under AppData/ProgramData,
#           cache-pattern hits, installed software list, pkg-manager caches.
# NEVER collects: file names, file contents, user name (paths are masked).
# Usage:  powershell -NoProfile -ExecutionPolicy Bypass -File collect-rules.ps1
#         add -IncludeDrives to also scan drive roots (2 levels, >=1GB dirs)
#         for knowledge-base clues (disk-assassin identification). Optional
#         because top-level folder names on data drives may reveal project
#         or company names - contributor decides.
# Output: Desktop\diskbutler-rule-report-<timestamp>.md / .json
# =====================================================================
param([switch]$IncludeDrives)
$ErrorActionPreference = 'SilentlyContinue'
$sw = [Diagnostics.Stopwatch]::StartNew()

# ---------- helpers ----------
function Get-DirSize([string]$p) {
    $s = (Get-ChildItem -LiteralPath $p -Recurse -File -Force -ErrorAction SilentlyContinue |
          Measure-Object -Property Length -Sum).Sum
    if ($null -eq $s) { 0 } else { [long]$s }
}
function ToMB([long]$b) { [math]::Round($b / 1MB, 1) }
function Mask([string]$path) {
    $path -replace [regex]::Escape($env:USERPROFILE), '%USERPROFILE%' `
          -replace [regex]::Escape($env:USERNAME), '%USER%'
}

# cache-like child dir patterns (case-insensitive exact or wildcard)
$cachePatterns = @('Cache','Code Cache','GPUCache','CachedData','CacheStorage','ShaderCache',
    'DawnCache','Crashpad','blob_storage','Service Worker','logs','log','Temp','tmp',
    'CrashDumps','Dumps','pending','staging')
$updaterPattern = '*updater*'
# personal-data red flags: NEVER become cleanup rules
$personalPatterns = @('*download*','*document*','*desktop*','*picture*','*photo*','*video*','*music*')

function Analyze-Root([string]$rootPath, [string]$rootLabel, [int]$minMB) {
    $out = @()
    $dirs = Get-ChildItem -LiteralPath $rootPath -Directory -Force -ErrorAction SilentlyContinue
    $n = 0
    foreach ($d in $dirs) {
        $n++
        Write-Progress -Activity ("Scanning " + $rootLabel) -Status $d.Name -PercentComplete ([math]::Min(100, $n * 100 / [math]::Max(1,$dirs.Count)))
        $size = Get-DirSize $d.FullName
        if ((ToMB $size) -lt $minMB) { continue }
        # inspect children + grandchildren (2 levels) for cache-like names
        $children = Get-ChildItem -LiteralPath $d.FullName -Directory -Force -Depth 1 -ErrorAction SilentlyContinue
        $hits = @()
        foreach ($c in $children) {
            $rel = $c.FullName.Substring($d.FullName.Length + 1)
            foreach ($p in $cachePatterns) {
                if ($c.Name -ieq $p) { $hits += $rel; break }
            }
            if ($c.Name -ilike $updaterPattern) { $hits += $rel }
        }
        $flags = @()
        foreach ($pp in $personalPatterns) {
            if ($d.Name -ilike $pp) { $flags += 'PERSONAL-DO-NOT-ADD'; break }
        }
        if ($d.Name -ilike $updaterPattern) { $flags += 'UPDATER-RESIDUE' }
        $out += [pscustomobject]@{
            root    = $rootLabel
            name    = $d.Name
            sizeMB  = (ToMB $size)
            cacheHits = (($hits | Select-Object -Unique | Select-Object -First 8) -join '; ')
            flags   = ($flags -join '; ')
        }
    }
    Write-Progress -Activity ("Scanning " + $rootLabel) -Completed
    $out | Sort-Object sizeMB -Descending
}

Write-Output 'DiskButler Rule Collector v1.0'
Write-Output 'Collecting... this may take a few minutes (recursive size scan).'

# ---------- 1. machine summary (no hostname, no username) ----------
$os = Get-CimInstance Win32_OperatingSystem
$disks = Get-CimInstance Win32_LogicalDisk -Filter "DriveType=3" | ForEach-Object {
    [pscustomobject]@{ drive = $_.DeviceID; totalGB = [math]::Round($_.Size/1GB,1); freeGB = [math]::Round($_.FreeSpace/1GB,1) }
}
$summary = [pscustomobject]@{
    reportVersion = '1.0'
    generatedAt   = (Get-Date).ToString('yyyy-MM-dd HH:mm')
    osVersion     = $os.Caption + ' ' + $os.Version
    disks         = $disks
}

# ---------- 2. installed software ----------
$regPaths = @(
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
)
$software = Get-ItemProperty -Path $regPaths -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName } |
    ForEach-Object { [pscustomobject]@{
        name = $_.DisplayName
        sizeMB = if ($_.EstimatedSize) { [math]::Round($_.EstimatedSize / 1024, 1) } else { $null }
    } } |
    Sort-Object name -Unique

# ---------- 3. AppData / ProgramData top-level analysis ----------
$localRows   = Analyze-Root $env:LOCALAPPDATA '%LOCALAPPDATA%' 10
$roamingRows = Analyze-Root $env:APPDATA      '%APPDATA%'      10
$progRows    = Analyze-Root $env:ProgramData  '%ProgramData%'  50

# ---------- 4. package manager caches ----------
$pkgCandidates = @(
    @{ name='npm';    path="$env:LOCALAPPDATA\npm-cache" },
    @{ name='npm2';   path="$env:APPDATA\npm-cache" },
    @{ name='pip';    path="$env:LOCALAPPDATA\pip\cache" },
    @{ name='yarn';   path="$env:LOCALAPPDATA\Yarn\Cache" },
    @{ name='pnpm';   path="$env:LOCALAPPDATA\pnpm\store" },
    @{ name='nuget';  path="$env:USERPROFILE\.nuget\packages" },
    @{ name='gradle'; path="$env:USERPROFILE\.gradle\caches" },
    @{ name='maven';  path="$env:USERPROFILE\.m2\repository" },
    @{ name='cargo';  path="$env:USERPROFILE\.cargo\registry" },
    @{ name='go';     path="$env:USERPROFILE\go\pkg\mod\cache" },
    @{ name='conda';  path="$env:USERPROFILE\miniconda3\pkgs" },
    @{ name='conda2'; path="$env:USERPROFILE\anaconda3\pkgs" }
)
$pkgRows = @()
foreach ($pc in $pkgCandidates) {
    if (Test-Path $pc.path) {
        $pkgRows += [pscustomobject]@{ manager = $pc.name; path = (Mask $pc.path); sizeMB = (ToMB (Get-DirSize $pc.path)) }
    }
}

# ---------- 4b. optional: drive roots, 2 levels, >=1GB (knowledge-base clues) ----------
$driveRows = @()
if ($IncludeDrives) {
    Write-Output 'IncludeDrives: scanning drive roots (this adds a few minutes)...'
    $fixedDrives = Get-CimInstance Win32_LogicalDisk -Filter "DriveType=3" | ForEach-Object { $_.DeviceID }
    foreach ($dv in $fixedDrives) {
        $rootDirs = Get-ChildItem -LiteralPath ($dv + '\') -Directory -Force -ErrorAction SilentlyContinue
        $n = 0
        foreach ($rd in $rootDirs) {
            $n++
            Write-Progress -Activity ("Scanning drive " + $dv) -Status $rd.Name -PercentComplete ([math]::Min(100, $n * 100 / [math]::Max(1,$rootDirs.Count)))
            $sz = Get-DirSize $rd.FullName
            if ($sz -lt 1GB) { continue }
            $driveRows += [pscustomobject]@{
                drive = $dv; depth = 1
                path  = (Mask ($dv + '\' + $rd.Name))
                sizeGB = [math]::Round($sz / 1GB, 2)
            }
            # level 2: children >= 1GB of a selected level-1 dir
            $subs = Get-ChildItem -LiteralPath $rd.FullName -Directory -Force -ErrorAction SilentlyContinue
            foreach ($sd in $subs) {
                $ssz = Get-DirSize $sd.FullName
                if ($ssz -ge 1GB) {
                    $driveRows += [pscustomobject]@{
                        drive = $dv; depth = 2
                        path  = (Mask ($dv + '\' + $rd.Name + '\' + $sd.Name))
                        sizeGB = [math]::Round($ssz / 1GB, 2)
                    }
                }
            }
        }
        Write-Progress -Activity ("Scanning drive " + $dv) -Completed
    }
}

# ---------- 5. assemble report ----------
$stamp = Get-Date -Format 'yyyyMMdd-HHmm'
$desktop = [Environment]::GetFolderPath('Desktop')
$mdPath   = Join-Path $desktop ("diskbutler-rule-report-" + $stamp + ".md")
$jsonPath = Join-Path $desktop ("diskbutler-rule-report-" + $stamp + ".json")

$report = [pscustomobject]@{
    summary  = $summary
    software = $software
    localAppData = $localRows
    roamingAppData = $roamingRows
    programData = $progRows
    packageCaches = $pkgRows
    driveTopDirs = $driveRows
}
$report | ConvertTo-Json -Depth 6 | Out-File -FilePath $jsonPath -Encoding utf8

$md = @()
$md += '# DiskButler Rule Collection Report'
$md += ''
$md += '> Privacy: no file names, no file contents, no user name. Review before sharing.'
$md += ''
$md += '## Machine Summary'
$md += '- Generated: ' + $summary.generatedAt
$md += '- OS: ' + $summary.osVersion
foreach ($dk in $disks) { $md += ('- Disk ' + $dk.drive + ' total ' + $dk.totalGB + ' GB, free ' + $dk.freeGB + ' GB') }
$md += ''
$md += '## Installed Software (' + $software.Count + ')'
$md += ''
foreach ($s in $software) {
    $sz = if ($s.sizeMB) { ' (' + $s.sizeMB + ' MB)' } else { '' }
    $md += ('- ' + $s.name + $sz)
}
foreach ($section in @(
    @{ title='%LOCALAPPDATA% Top-Level Directories (>=10MB)'; rows=$localRows },
    @{ title='%APPDATA% Top-Level Directories (>=10MB)';      rows=$roamingRows },
    @{ title='%ProgramData% Top-Level Directories (>=50MB)';  rows=$progRows })) {
    $md += ''
    $md += ('## ' + $section.title)
    $md += ''
    $md += '| Directory | Size (MB) | Cache-like children | Flags |'
    $md += '|---|---|---|---|'
    foreach ($r in $section.rows) {
        $md += ('| ' + $r.name + ' | ' + $r.sizeMB + ' | ' + $r.cacheHits + ' | ' + $r.flags + ' |')
    }
}
$md += ''
$md += '## Package Manager Caches'
$md += ''
$md += '| Manager | Path | Size (MB) |'
$md += '|---|---|---|'
foreach ($p in $pkgRows) { $md += ('| ' + $p.manager + ' | ' + $p.path + ' | ' + $p.sizeMB + ' |') }
if ($IncludeDrives) {
    $md += ''
    $md += '## Drive Top Directories (2 levels, >=1GB) - knowledge-base clues'
    $md += ''
    $md += '| Path | Size (GB) |'
    $md += '|---|---|'
    foreach ($r in $driveRows) {
        $indent = if ($r.depth -eq 2) { '&nbsp;&nbsp;&nbsp;&nbsp;' } else { '' }
        $md += ('| ' + $indent + $r.path + ' | ' + $r.sizeGB + ' |')
    }
}
$md += ''
$md += ('_Scan took ' + [math]::Round($sw.Elapsed.TotalMinutes,1) + ' minutes._')
$md -join "`r`n" | Out-File -FilePath $mdPath -Encoding utf8

Write-Output ''
Write-Output ('Done in ' + [math]::Round($sw.Elapsed.TotalMinutes,1) + ' min.')
Write-Output ('Report (readable): ' + $mdPath)
Write-Output ('Report (machine):  ' + $jsonPath)
Write-Output 'Please REVIEW the report before sharing it.'
