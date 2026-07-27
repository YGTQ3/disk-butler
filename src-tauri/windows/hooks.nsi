; C盘管家 NSIS 安装钩子：注册/注销后台扫描服务（DiskButlerScanSvc）。
; 安装器以 perMachine 模式运行（已提权），此处无需再次 UAC。

!macro NSIS_HOOK_PREINSTALL
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
