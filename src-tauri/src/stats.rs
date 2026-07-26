//! 累计清理统计：每次清理（常规/深度）后累加释放量并本地持久化，
//! 给用户一点"这些空间都是我拿回来的"成就感。只存在本地，不上传。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CleanupStats {
    /// 历史累计释放字节数
    pub total_freed: u64,
    /// 累计清理次数（常规 + 深度）
    pub total_runs: u32,
    /// 最近一次清理时刻（Unix 秒）
    pub last_at: u64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn stats_file(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("cleanup-stats.json"))
}

/// 读取累计统计；文件不存在或损坏时返回全零默认值。
pub fn load(app: &tauri::AppHandle) -> CleanupStats {
    stats_file(app)
        .and_then(|f| std::fs::read(f).ok())
        .and_then(|b| serde_json::from_slice::<CleanupStats>(&b).ok())
        .unwrap_or_default()
}

/// 记录一次清理（失败静默：统计是锦上添花，不能影响清理主流程）。
pub fn record(app: &tauri::AppHandle, freed: u64) {
    let Some(file) = stats_file(app) else {
        return;
    };
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut s = load(app);
    s.total_freed = s.total_freed.saturating_add(freed);
    s.total_runs = s.total_runs.saturating_add(1);
    s.last_at = now_secs();
    if let Ok(json) = serde_json::to_vec(&s) {
        let _ = std::fs::write(&file, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_serde_roundtrip_camel_case() {
        let s = CleanupStats {
            total_freed: 6_100_000_000,
            total_runs: 3,
            last_at: 1234567890,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("totalFreed"));
        assert!(json.contains("totalRuns"));
        let back: CleanupStats = serde_json::from_str(&json).unwrap();
        assert_eq!(back.total_freed, 6_100_000_000);
        assert_eq!(back.total_runs, 3);
    }

    #[test]
    fn stats_default_on_bad_json() {
        let back = serde_json::from_slice::<CleanupStats>(b"{}").unwrap();
        assert_eq!(back.total_freed, 0);
        assert_eq!(back.total_runs, 0);
    }
}
