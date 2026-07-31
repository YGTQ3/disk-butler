fn main() {
    // 未定稿大类功能的编译期门控：环境变量 DISKBUTLER_FEATURE_BLOATWARE=1 才开启，
    // 默认关闭——「软件体检」相关模块与命令不编译进公开包（攻击面为零）。
    // 前端 vite define 的 __FEATURE_BLOATWARE__ 读同一环境变量，一个开关同时管前后端。
    println!("cargo:rerun-if-env-changed=DISKBUTLER_FEATURE_BLOATWARE");
    println!("cargo:rustc-check-cfg=cfg(feature_bloatware)");
    if std::env::var("DISKBUTLER_FEATURE_BLOATWARE").as_deref() == Ok("1") {
        println!("cargo:rustc-cfg=feature_bloatware");
    }
    tauri_build::build()
}
