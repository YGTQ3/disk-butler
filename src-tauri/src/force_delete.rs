//! 强力删除：定位占用文件/文件夹的进程，可选终止后删除。
//! 定位主用 NtQueryInformationFile(FileProcessIdsUsingFileInformation)——Restart Manager 内部同款，
//! 打开目标句柄直接问内核谁在用它，确定/快速/不卡死；再叠加 Restart Manager 兜底。
//! 终止优先 TerminateProcess（普通权限），被拒的进程再用一次提权 taskkill 兜底。

use serde::Serialize;
use std::ffi::c_void;
use std::path::Path;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 占用某路径的进程信息。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LockingProcess {
    pub pid: u32,
    pub name: String,
    /// 系统关键进程：终止会导致系统崩溃/重启，前端禁止勾选、后端也拒绝终止。
    pub is_critical: bool,
}

/// 系统关键进程名（小写，可含 .exe）：一律禁止终止。
/// 含 explorer（桌面外壳，关了会黑屏）与会导致系统崩溃/重启的核心进程。
fn is_critical_name(name_lower: &str) -> bool {
    let n = name_lower.strip_suffix(".exe").unwrap_or(name_lower);
    matches!(
        n,
        "system"
            | "system idle process"
            | "smss"
            | "csrss"
            | "wininit"
            | "winlogon"
            | "services"
            | "lsass"
            | "lsaiso"
            | "fontdrvhost"
            | "dwm"
            | "svchost"
            | "explorer"
            | "taskhostw"
            | "ctfmon"
            | "sihost"
    )
}

fn current_pid() -> u32 {
    std::process::id()
}

fn wstr_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// 查找占用指定文件/文件夹的进程。
/// 主方法：NtQueryInformationFile(FileProcessIdsUsingFileInformation)——打开目标句柄直接问内核
/// “哪些进程在用它”（Restart Manager 内部同款），确定性、无需全系统扫描、不卡死；
/// 目录会额外查其中文件。再叠加 Restart Manager 兜底。
pub fn find_lockers(path: &str) -> Vec<LockingProcess> {
    use std::collections::HashMap;
    let p = Path::new(path);
    if !p.exists() {
        return Vec::new();
    }

    // 目标本体 +（若是目录）其中文件，逐个查占用
    let mut targets: Vec<String> = vec![path.to_string()];
    if p.is_dir() {
        collect_files(p, &mut targets, 400);
    }

    let mut map: HashMap<u32, LockingProcess> = HashMap::new();
    let me = current_pid();

    // 1) 主方法：文件对象的占用进程 PID 列表
    for t in &targets {
        for pid in nt_file_lockers(t) {
            if pid == 0 || pid == me || map.contains_key(&pid) {
                continue;
            }
            if let Some(name) = process_name(pid) {
                let is_critical = is_critical_name(&name.to_lowercase());
                map.insert(
                    pid,
                    LockingProcess {
                        pid,
                        name,
                        is_critical,
                    },
                );
            }
        }
    }

    // 2) Restart Manager 兜底（登记文件，补充主方法可能遗漏的锁定者）
    for lp in rm_find(&targets) {
        map.entry(lp.pid).or_insert(lp);
    }

    let mut result: Vec<LockingProcess> = map.into_values().collect();
    result.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.pid.cmp(&b.pid))
    });
    result
}

/// 递归收集目录下的文件路径（限量，避免病态大目录拖慢）。
fn collect_files(dir: &Path, out: &mut Vec<String>, cap: usize) {
    if out.len() >= cap {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        if out.len() >= cap {
            return;
        }
        let path = e.path();
        if path.is_dir() {
            collect_files(&path, out, cap);
        } else {
            out.push(path.to_string_lossy().to_string());
        }
    }
}

/// Restart Manager 查询占用给定文件的进程。
fn rm_find(files: &[String]) -> Vec<LockingProcess> {
    use windows_sys::Win32::System::RestartManager::{
        RmEndSession, RmGetList, RmRegisterResources, RmStartSession, RM_PROCESS_INFO,
    };
    unsafe {
        let mut session: u32 = 0;
        let mut key = [0u16; 64]; // CCH_RM_SESSION_KEY(32)+1，放大冗余
        if RmStartSession(&mut session, 0, key.as_mut_ptr()) != 0 {
            return Vec::new();
        }
        // 组装文件名宽字符指针数组（wides 必须存活到调用结束）
        let wides: Vec<Vec<u16>> = files
            .iter()
            .map(|f| f.encode_utf16().chain(std::iter::once(0)).collect())
            .collect();
        let ptrs: Vec<*const u16> = wides.iter().map(|w| w.as_ptr()).collect();
        let reg = RmRegisterResources(
            session,
            ptrs.len() as u32,
            ptrs.as_ptr(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
        );
        if reg != 0 {
            RmEndSession(session);
            return Vec::new();
        }
        // 先查数量，再取列表
        let mut needed: u32 = 0;
        let mut count: u32 = 0;
        let mut reason: u32 = 0;
        RmGetList(
            session,
            &mut needed,
            &mut count,
            std::ptr::null_mut(),
            &mut reason,
        );
        let mut result: Vec<LockingProcess> = Vec::new();
        if needed > 0 {
            let mut infos: Vec<RM_PROCESS_INFO> = vec![std::mem::zeroed(); needed as usize];
            count = needed;
            let r = RmGetList(
                session,
                &mut needed,
                &mut count,
                infos.as_mut_ptr(),
                &mut reason,
            );
            if r == 0 {
                let me = current_pid();
                for info in infos.iter().take(count as usize) {
                    let pid = info.Process.dwProcessId;
                    if pid == 0 || pid == me {
                        continue;
                    }
                    let name = wstr_to_string(&info.strAppName);
                    let is_critical = is_critical_name(&name.to_lowercase());
                    result.push(LockingProcess {
                        pid,
                        name,
                        is_critical,
                    });
                }
            }
        }
        RmEndSession(session);
        // 去重（同一进程可能锁多个文件）
        result.sort_by_key(|p| p.pid);
        result.dedup_by_key(|p| p.pid);
        result
    }
}

/// 解除占用并删除路径。升级策略：关句柄+立即删（抢在监视器重开前，重试）→ 结束整个占用应用 → 再删。
/// to_recycle=true 走回收站(可恢复)，否则永久删除。
pub fn force_delete(path: &str, pids: Vec<u32>, to_recycle: bool) -> Result<(), String> {
    let target_nt = dos_to_nt(&path.trim_end_matches(['\\', '/']).to_lowercase());

    // 1) 关句柄 + 立即删（不 sleep，抢在监视器重新打开前），重试几次——顺利时无需杀进程
    if let Some(tn) = target_nt.as_ref() {
        let sub = format!("{}\\", tn);
        for _ in 0..4 {
            let mut closed = 0usize;
            for &pid in &pids {
                if pid == 0 || pid == current_pid() {
                    continue;
                }
                closed += close_handles_to(pid, tn, &sub);
            }
            if delete_path(path, to_recycle).is_ok() {
                return Ok(());
            }
            if closed == 0 {
                break; // 没句柄可关，重试无意义
            }
        }
    }

    // 2) 仍被占用 → 结束整个占用应用（按映像名结束其全部进程，等价资源监视器/任务管理器“结束进程树”），再删
    kill_locker_apps(&pids)?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    delete_path(path, to_recycle)
}

/// 结束占用文件的整个应用：把选中进程的（非关键）映像名整体 taskkill /F /T（含其全部进程及子树），
/// 等价资源监视器“结束进程”。只杀部分子进程会被主进程重开，故按映像名整体结束才可靠。
fn kill_locker_apps(pids: &[u32]) -> Result<(), String> {
    use std::collections::HashSet;
    use std::os::windows::process::CommandExt;
    let mut names: HashSet<String> = HashSet::new();
    for &pid in pids {
        if pid == 0 || pid == current_pid() {
            continue;
        }
        if let Some(name) = process_name(pid) {
            // 防御：绝不结束系统关键进程（如 explorer，关了会黑屏）
            if is_critical_name(&name.to_lowercase()) {
                continue;
            }
            names.insert(name);
        }
    }
    if names.is_empty() {
        return Ok(());
    }
    let mut args: Vec<String> = vec!["/F".to_string(), "/T".to_string()];
    for n in &names {
        args.push("/IM".to_string());
        args.push(n.clone());
    }
    // 普通权限 taskkill 对同用户进程即可结束；部分失败不致命（子进程随主进程一起结束）
    let _ = std::process::Command::new("taskkill")
        .args(&args)
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    Ok(())
}

/// 删除路径：回收站(可恢复) 或 永久删除；永久删除失败时提权兜底。
fn delete_path(path: &str, to_recycle: bool) -> Result<(), String> {
    let p = Path::new(path);
    if !p.exists() {
        return Ok(()); // 已不存在，视作成功
    }
    if to_recycle {
        return send_to_recycle(path);
    }
    let r = if p.is_dir() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    };
    match r {
        Ok(()) => Ok(()),
        Err(_) => elevated_delete(path, p.is_dir()), // 受保护 → 提权删除
    }
}

/// 删入回收站（可恢复），文件/文件夹通用。
fn send_to_recycle(path: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::{SHFileOperationW, SHFILEOPSTRUCTW};
    const FO_DELETE: u32 = 0x0003;
    const FOF_SILENT: u16 = 0x0004;
    const FOF_NOCONFIRMATION: u16 = 0x0010;
    const FOF_ALLOWUNDO: u16 = 0x0040;
    const FOF_NOERRORUI: u16 = 0x0400;

    let mut from: Vec<u16> = path.encode_utf16().collect();
    from.push(0);
    from.push(0);

    let mut op = SHFILEOPSTRUCTW {
        hwnd: std::ptr::null_mut(),
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI,
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    let ret = unsafe { SHFileOperationW(&mut op) };
    if ret == 0 && op.fAnyOperationsAborted == 0 {
        Ok(())
    } else {
        Err(format!("移入回收站失败 (code={})", ret))
    }
}

/// 提权永久删除（受保护路径兜底，单次 UAC，隐藏窗口）。
fn elevated_delete(path: &str, is_dir: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let cmd = if is_dir {
        format!(r#"rmdir /s /q "{}""#, path.trim_end_matches('\\'))
    } else {
        format!(r#"del /f /q "{}""#, path)
    };
    // 作为单个参数传给 cmd /c（PowerShell 单引号字符串，内部单引号转义为两个）
    let quoted = format!("'{}'", cmd.replace('\'', "''"));
    let ps = format!(
        r#"$p = Start-Process -Verb RunAs -WindowStyle Hidden -Wait -PassThru -FilePath cmd.exe -ArgumentList '/c',{}; exit $p.ExitCode"#,
        quoted
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("提权删除失败：{}", e))?;
    match status.code().unwrap_or(-1) {
        0 => {
            if Path::new(path).exists() {
                Err("删除未完成（文件可能仍被占用）。".to_string())
            } else {
                Ok(())
            }
        }
        1 => Err("已取消管理员授权，未删除。".to_string()),
        c => Err(format!("提权删除未完成（退出码 {}）。", c)),
    }
}

/// 计划在下次重启时删除（MoveFileEx + MOVEFILE_DELAY_UNTIL_REBOOT，微软更新占用文件同款）。
/// 目录需递归登记（深层优先）；写 PendingFileRenameOperations 需管理员，故提权执行，弹一次 UAC。
#[allow(dead_code)]
fn schedule_reboot_delete(path: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // 内层脚本：递归深层优先登记每个文件/子目录，最后登记目标本身（MOVEFILE_DELAY_UNTIL_REBOOT=4）
    let script = format!(
        "Add-Type -Namespace N -Name F -MemberDefinition '[DllImport(\"kernel32.dll\",CharSet=CharSet.Unicode,SetLastError=true)] public static extern bool MoveFileEx(string a, string b, int f);'\r\n\
$t = '{}'\r\n\
Get-ChildItem -LiteralPath $t -Recurse -Force -ErrorAction SilentlyContinue | Sort-Object {{ $_.FullName.Length }} -Descending | ForEach-Object {{ [N.F]::MoveFileEx($_.FullName, $null, 4) | Out-Null }}\r\n\
[N.F]::MoveFileEx($t, $null, 4) | Out-Null\r\n",
        path.replace('\'', "''")
    );
    let tmp = std::env::temp_dir().join(format!("db-reboot-del-{}.ps1", std::process::id()));
    // UTF-8 BOM 保证 PowerShell 正确读取中文路径
    let mut bytes = vec![0xEFu8, 0xBB, 0xBF];
    bytes.extend_from_slice(script.as_bytes());
    std::fs::write(&tmp, &bytes).map_err(|e| format!("写入计划删除脚本失败：{}", e))?;
    let ps = format!(
        r#"$p = Start-Process -Verb RunAs -WindowStyle Hidden -Wait -PassThru -FilePath powershell.exe -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','"{}"'; exit $p.ExitCode"#,
        tmp.display()
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("提权计划删除失败：{}", e))?;
    let _ = std::fs::remove_file(&tmp);
    match status.code().unwrap_or(-1) {
        0 => Ok(()),
        1 => Err("已取消管理员授权，未计划删除。".to_string()),
        c => Err(format!("计划重启删除未完成（退出码 {}）。", c)),
    }
}

// ---------- NT 文件占用查询（Restart Manager 内部同款：直接问内核谁在用该文件对象） ----------

#[link(name = "ntdll")]
extern "system" {
    fn NtQueryInformationFile(
        handle: *mut c_void,
        io: *mut IoStatusBlock,
        info: *mut c_void,
        len: u32,
        class: u32,
    ) -> i32;
}

#[repr(C)]
struct IoStatusBlock {
    status_or_pointer: usize,
    information: usize,
}

/// 打开一个仅用于查询的句柄（读属性 + 全共享，即便文件/目录被占用也能打开）。失败返回 null。
fn open_query_handle(path: &str) -> *mut c_void {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    let wide: Vec<u16> = Path::new(path)
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let h = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS, // 打开目录句柄必需
            std::ptr::null_mut(),
        )
    };
    if h == INVALID_HANDLE_VALUE {
        std::ptr::null_mut()
    } else {
        h
    }
}

/// 查询正在使用某文件/目录的进程 PID 列表（FileProcessIdsUsingFileInformation=47）。
/// 打开目标句柄后由内核直接返回，确定、快速、无需枚举全系统句柄、不卡死。
fn nt_file_lockers(path: &str) -> Vec<u32> {
    use windows_sys::Win32::Foundation::CloseHandle;
    const FILE_PROCESS_IDS_USING_FILE_INFORMATION: u32 = 47;

    let h = open_query_handle(path);
    if h.is_null() {
        return Vec::new();
    }
    let mut result = Vec::new();
    // 缓冲区：count(4字节+对齐) + PID 数组，4KB 足以容纳数百个 PID
    let mut buf = vec![0u8; 4096];
    let mut io = IoStatusBlock {
        status_or_pointer: 0,
        information: 0,
    };
    let status = unsafe {
        NtQueryInformationFile(
            h,
            &mut io,
            buf.as_mut_ptr() as *mut c_void,
            buf.len() as u32,
            FILE_PROCESS_IDS_USING_FILE_INFORMATION,
        )
    };
    unsafe { CloseHandle(h) };
    if status < 0 {
        return result;
    }
    // FILE_PROCESS_IDS_USING_FILE_INFORMATION: ULONG NumberOfProcessIdsInList; ULONG_PTR ProcessIdList[];
    // 64 位下 ProcessIdList 按 8 字节对齐，紧跟在偏移 8（=size_of::<usize>）处
    unsafe {
        let count = *(buf.as_ptr() as *const u32) as usize;
        let list = (buf.as_ptr() as usize + std::mem::size_of::<usize>()) as *const usize;
        let max = (buf.len() - std::mem::size_of::<usize>()) / std::mem::size_of::<usize>();
        for i in 0..count.min(max) {
            let pid = *list.add(i) as u32;
            if pid != 0 {
                result.push(pid);
            }
        }
    }
    result
}

/// 取进程可执行文件名（如 Code.exe）。
fn process_name(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 260];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        let full = String::from_utf16_lossy(&buf[..size as usize]);
        Some(full.rsplit(['\\', '/']).next().unwrap_or(&full).to_string())
    }
}

// ---------- 关闭远程句柄解除占用（LockHunter/Unlocker 同款：不杀进程即可释放锁） ----------

#[link(name = "ntdll")]
extern "system" {
    fn NtQueryObject(handle: *mut c_void, class: u32, info: *mut c_void, len: u32, ret: *mut u32)
        -> i32;
    fn NtQueryInformationProcess(
        handle: *mut c_void,
        class: u32,
        info: *mut c_void,
        len: u32,
        ret: *mut u32,
    ) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn QueryDosDeviceW(device: *const u16, target: *mut u16, max: u32) -> u32;
}

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}

#[repr(C)]
struct ProcHandleEntry {
    handle_value: usize,
    handle_count: usize,
    pointer_count: usize,
    granted_access: u32,
    object_type_index: u32,
    handle_attributes: u32,
    reserved: u32,
}

/// DOS 路径（小写，如 d:\a\b）转 NT 设备路径前缀（小写，如 \device\harddiskvolume3\a\b）。
fn dos_to_nt(dos_lower: &str) -> Option<String> {
    if dos_lower.len() < 2 || dos_lower.as_bytes()[1] != b':' {
        return None;
    }
    let drive = &dos_lower[..2];
    let rest = &dos_lower[2..];
    let wdrive: Vec<u16> = drive.encode_utf16().chain(std::iter::once(0)).collect();
    let mut buf = [0u16; 512];
    let n = unsafe { QueryDosDeviceW(wdrive.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    if n == 0 {
        return None;
    }
    let dev = wstr_to_string(&buf);
    if dev.is_empty() {
        return None;
    }
    Some(format!("{}{}", dev.to_lowercase(), rest))
}

/// 查询本进程内复制句柄的对象名（\Device\HarddiskVolumeX\...）。
unsafe fn query_object_name(h: *mut c_void) -> Option<String> {
    const OBJECT_NAME_INFORMATION: u32 = 1;
    let mut buf = [0u8; 4096];
    let mut ret: u32 = 0;
    let status = NtQueryObject(
        h,
        OBJECT_NAME_INFORMATION,
        buf.as_mut_ptr() as *mut c_void,
        buf.len() as u32,
        &mut ret,
    );
    if status < 0 {
        return None;
    }
    let us = &*(buf.as_ptr() as *const UnicodeString);
    if us.length == 0 || us.buffer.is_null() {
        return None;
    }
    let slice = std::slice::from_raw_parts(us.buffer, (us.length / 2) as usize);
    Some(String::from_utf16_lossy(slice))
}

/// 枚举指定进程的句柄（NtQueryInformationProcess(ProcessHandleInformation=51)）。返回 (句柄值, 授权掌码)。
fn enum_process_handles(proc: *mut c_void) -> Vec<(usize, u32)> {
    const PROCESS_HANDLE_INFORMATION: u32 = 51;
    const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004u32 as i32;
    let mut cap: usize = 64 * 1024;
    loop {
        let mut buf = vec![0u8; cap];
        let mut ret: u32 = 0;
        let status = unsafe {
            NtQueryInformationProcess(
                proc,
                PROCESS_HANDLE_INFORMATION,
                buf.as_mut_ptr() as *mut c_void,
                cap as u32,
                &mut ret,
            )
        };
        if status == STATUS_INFO_LENGTH_MISMATCH {
            cap = (cap * 2).min(64 << 20);
            if cap >= (64 << 20) {
                return Vec::new();
            }
            continue;
        }
        if status < 0 {
            return Vec::new();
        }
        unsafe {
            let count = *(buf.as_ptr() as *const usize);
            let entries =
                (buf.as_ptr() as usize + 2 * std::mem::size_of::<usize>()) as *const ProcHandleEntry;
            let mut out = Vec::with_capacity(count.min(200_000));
            for i in 0..count.min(200_000) {
                let e = &*entries.add(i);
                out.push((e.handle_value, e.granted_access));
            }
            return out;
        }
    }
}

/// 关闭目标进程中指向 target_nt（或其子路径）的句柄，解除占用而不杀进程。返回关闭数。
fn close_handles_to(pid: u32, target_nt: &str, target_nt_sub: &str) -> usize {
    use windows_sys::Win32::Foundation::{
        CloseHandle, DuplicateHandle, DUPLICATE_CLOSE_SOURCE, DUPLICATE_SAME_ACCESS,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcess, PROCESS_DUP_HANDLE};
    let src = unsafe { OpenProcess(PROCESS_DUP_HANDLE, 0, pid) };
    if src.is_null() {
        return 0;
    }
    let cur = unsafe { GetCurrentProcess() };
    let mut closed = 0usize;
    for (hv, ga) in enum_process_handles(src) {
        if ga == 0x0012_019F {
            continue; // 跳过会让 NtQueryObject 卡死的同步句柄
        }
        // 先复制一份来查名字
        let mut dup: *mut c_void = std::ptr::null_mut();
        let ok =
            unsafe { DuplicateHandle(src, hv as *mut c_void, cur, &mut dup, 0, 0, DUPLICATE_SAME_ACCESS) };
        if ok == 0 || dup.is_null() {
            continue;
        }
        let name = unsafe { query_object_name(dup) };
        unsafe { CloseHandle(dup) };
        let Some(name) = name else {
            continue;
        };
        let nl = name.to_lowercase();
        if nl == target_nt || nl.starts_with(target_nt_sub) {
            // 命中：DUPLICATE_CLOSE_SOURCE 关闭源进程里的该句柄
            let mut d2: *mut c_void = std::ptr::null_mut();
            let ok2 = unsafe {
                DuplicateHandle(src, hv as *mut c_void, cur, &mut d2, 0, 0, DUPLICATE_CLOSE_SOURCE)
            };
            if ok2 != 0 {
                if !d2.is_null() {
                    unsafe { CloseHandle(d2) };
                }
                closed += 1;
            }
        }
    }
    unsafe { CloseHandle(src) };
    closed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_names_detected() {
        assert!(is_critical_name("csrss.exe"));
        assert!(is_critical_name("lsass"));
        assert!(is_critical_name("system"));
        assert!(is_critical_name("explorer.exe"));
        assert!(!is_critical_name("chrome.exe"));
        assert!(!is_critical_name("qq.exe"));
    }

    #[test]
    fn wstr_stops_at_nul() {
        let buf = [0x41u16, 0x42, 0x00, 0x43];
        assert_eq!(wstr_to_string(&buf), "AB");
    }
}
