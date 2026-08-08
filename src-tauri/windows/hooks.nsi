; C盘管家 NSIS 安装钩子：OS 版本预检 + WebView2 x64 预检 + 注册/注销后台扫描服务（DiskButlerScanSvc）。
; 安装器以 perMachine 模式运行（已提权），此处无需再次 UAC。

!include WinVer.nsh

; 读 PE 头判 exe 位深：${WV2_IS64} <exe路径> <输出寄存器>（1=x64，0=x86/读不了）
!macro WV2_IS64 UN WV2P OUT
  StrCpy ${OUT} 0
  FileOpen $8 "${WV2P}" r
  IfErrors wv2pe_done_${UN}
  FileSeek $8 0x3C SET
  FileReadByte $8 $7
  FileSeek $8 0x3D SET
  FileReadByte $8 $9
  IntOp $9 $9 << 8
  IntOp $7 $7 + $9
  FileSeek $8 0x3E SET
  FileReadByte $8 $9
  IntOp $9 $9 << 16
  IntOp $7 $7 + $9
  FileSeek $8 0x3F SET
  FileReadByte $8 $9
  IntOp $9 $9 << 24
  IntOp $7 $7 + $9   ; e_lfanew 是 4 字节小端 DWORD，少读会把 PE 头位置算错
  IntOp $7 $7 + 4    ; PE 签名 4 字节，Machine 字段紧随其后
  FileSeek $8 $7 SET
  FileReadByte $8 $7 ; Machine 低字节（读后位置自动后移）
  FileReadByte $8 $9 ; Machine 高字节
  IntOp $9 $9 << 8
  IntOp $7 $7 + $9   ; Machine 字段（小端）
  IntCmp $7 0x8664 0 wv2pe_done_${UN} wv2pe_done_${UN}
  StrCpy ${OUT} 1
  wv2pe_done_${UN}:
  FileClose $8
!macroend

!macro NSIS_HOOK_PREINSTALL
  ; OS 版本预检：本软件需要 Windows 10 及以上（WebView2 与 Rust 工具链均不支持 Win7/8），
  ; 老系统上给人话提示后中止，而不是装完打不开让用户瞎猜
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_ICONSTOP "很抱歉，本软件需要 Windows 10 或 Windows 11（64 位）系统。$\r$\n$\r$\n您的系统版本较低，无法运行本软件，安装已取消。"
    Abort
  ${EndIf}

  ; WebView2 x64 组件预检（反馈 P 修正）：注册表 pv 存在 ≠ x64 Runtime 可用。
  ; 典型故障机：只有随 32 位 Edge 附带的 x86 老版 Runtime，安装器按微软标准判"已装"
  ; 跳过自动安装 → 装完 x64 应用打不开。
  ; 实测目录结构：<根>\Application\<版本>\msedgewebview2.exe（位深需读 PE 头确认），
  ; 旧版固定版布局为 <版本>\EBWebView\x64\msedgewebview2.exe，两种都认。
  ReadRegStr $1 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ReadRegStr $2 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  StrCmp $1 "" 0 wv2_pv_ok
  StrCmp $2 "" wv2_no_pv wv2_pv_ok
  wv2_no_pv:
    ; pv 都不存在：安装器自带的 WebView2 引导安装会处理，不拦截
    Goto wv2_check_done
  wv2_pv_ok:
    ; pv 存在：逐个版本目录找 x64 实体（系统级 + 用户级两处安装位置）
    StrCpy $3 "" ; 置非空即视为找到可用 x64
    ; --- 系统级 ---
    FindFirst $5 $6 "$PROGRAMFILES32\Microsoft\EdgeWebView\Application\*"
    wv2_sys_loop:
      StrCmp $6 "" wv2_sys_end
      StrCmp $6 "." wv2_sys_next
      StrCmp $6 ".." wv2_sys_next
      ${If} ${FileExists} "$PROGRAMFILES32\Microsoft\EdgeWebView\Application\$6\EBWebView\x64\msedgewebview2.exe"
        StrCpy $3 "ok"
      ${Else}
        !insertmacro WV2_IS64 S1 "$PROGRAMFILES32\Microsoft\EdgeWebView\Application\$6\msedgewebview2.exe" $R9
        StrCmp $R9 "1" 0 +2
        StrCpy $3 "ok"
      ${EndIf}
      wv2_sys_next:
      FindNext $5 $6
      Goto wv2_sys_loop
    wv2_sys_end:
    FindClose $5
    StrCmp $3 "" "" wv2_check_done
    ; --- 用户级 ---
    FindFirst $5 $6 "$LOCALAPPDATA\Microsoft\EdgeWebView\Application\*"
    wv2_user_loop:
      StrCmp $6 "" wv2_user_end
      StrCmp $6 "." wv2_user_next
      StrCmp $6 ".." wv2_user_next
      ${If} ${FileExists} "$LOCALAPPDATA\Microsoft\EdgeWebView\Application\$6\EBWebView\x64\msedgewebview2.exe"
        StrCpy $3 "ok"
      ${Else}
        !insertmacro WV2_IS64 S2 "$LOCALAPPDATA\Microsoft\EdgeWebView\Application\$6\msedgewebview2.exe" $R9
        StrCmp $R9 "1" 0 +2
        StrCpy $3 "ok"
      ${EndIf}
      wv2_user_next:
      FindNext $5 $6
      Goto wv2_user_loop
    wv2_user_end:
    FindClose $5
    StrCmp $3 "" "" wv2_check_done
    ; pv 存在但只有 x86 组件：人话说明 + 引导下载 Evergreen（会自动补齐 64 位）
    ; 用户选"否"时不硬拦（用户自决），装完可能打不开时诊断脚本会给出同一结论
    MessageBox MB_ICONEXCLAMATION|MB_YESNO "检测到你的电脑只装了 32 位 WebView2，而本软件需要 64 位组件——$\r$\n这种情况装完可能打不开（不是安装包的问题）。$\r$\n$\r$\n建议先安装微软官方的 WebView2 Evergreen 引导程序（自动补齐 64 位组件），$\r$\n装完后重新运行本安装包。$\r$\n$\r$\n是否现在打开下载页面？" IDYES wv2_open_dl
    Goto wv2_check_done
    wv2_open_dl:
    ExecShell "open" "https://developer.microsoft.com/zh-cn/microsoft-edge/webview2/"
    Abort
  wv2_check_done:

  ; 覆盖安装前先停止旧服务，避免 exe 被进程锁定导致写入失败
  nsExec::ExecToLog 'sc.exe stop DiskButlerScanSvc'
  Pop $0
  Sleep 1500
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; 创建并启动只读 MFT 扫描服务；失败不阻塞安装（主程序会自动回退慢速扫描）
  nsExec::ExecToLog '"$INSTDIR\disk-butler-svc.exe" install'
  Pop $0
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; 停止并删除服务，保证卸载干净、不留后台进程
  nsExec::ExecToLog '"$INSTDIR\disk-butler-svc.exe" uninstall'
  Pop $0
!macroend
