// 宽字符控制台输出：中文经 WriteConsoleW 直写控制台缓冲区，完全绕开代码页
// （cmd 编码地狱的根治方案：不经过任何字节编码转换）。
// 输出被重定向（非控制台）时回退为 UTF-8 字节流。
use std::io::Write;
use windows_sys::Win32::System::Console::{
    GetConsoleMode, GetStdHandle, WriteConsoleW, STD_OUTPUT_HANDLE,
};

pub fn cprint(s: &str) {
    unsafe {
        let h = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0u32;
        if !h.is_null() && GetConsoleMode(h, &mut mode) != 0 {
            let wide: Vec<u16> = s.encode_utf16().collect();
            let mut written = 0u32;
            WriteConsoleW(
                h,
                wide.as_ptr() as *const _,
                wide.len() as u32,
                &mut written,
                std::ptr::null(),
            );
            return;
        }
    }
    let mut out = std::io::stdout();
    let _ = out.write_all(s.as_bytes());
    let _ = out.flush();
}

pub fn cprintln(s: &str) {
    cprint(s);
    cprint("\r\n");
}
