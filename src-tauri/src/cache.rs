//! 扫描结果缓存：扫描完成后把剪枝树（仅目录名+大小+分类元数据）保存到应用数据目录，
//! 下次启动秒开上次结果；界面明确标注扫描时间，更新永远由用户手动触发。

use crate::scan::TreeNode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanCache {
    /// 缓存格式版本（旧版/缺字段一律视为未命中，重扫自愈；issue#9 修复后清旧 #n 假节点缓存）
    pub version: u32,
    /// 扫描完成时刻（Unix 秒）
    pub scanned_at: u64,
    /// 扫描根（挂载点，如 "C:\\"）
    pub root: String,
    pub tree: TreeNode,
}

/// 当前缓存格式版本；需要统一作废旧缓存时 bump。
const SCAN_CACHE_VERSION: u32 = 1;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 每个盘一个缓存文件：scan-cache-C.json
fn cache_file(app: &tauri::AppHandle, root: &str) -> Option<PathBuf> {
    let dir = app.path().app_data_dir().ok()?;
    let letter: String = root
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect();
    if letter.is_empty() {
        return None;
    }
    Some(dir.join(format!("scan-cache-{}.json", letter)))
}

/// 保存扫描结果（失败静默：缓存是锦上添花，不能影响主流程）。
pub fn save(app: &tauri::AppHandle, root: &str, tree: &TreeNode) {
    let Some(file) = cache_file(app, root) else {
        return;
    };
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let cache = ScanCache {
        version: SCAN_CACHE_VERSION,
        scanned_at: now_secs(),
        root: root.to_string(),
        tree: tree.clone(),
    };
    if let Ok(json) = serde_json::to_vec(&cache) {
        let _ = std::fs::write(&file, json);
    }
}

/// 读取某个盘的缓存；文件不存在、损坏、版本不符或是空树（历史异常扫描的产物）返回 None。
pub fn load(app: &tauri::AppHandle, root: &str) -> Option<ScanCache> {
    let file = cache_file(app, root)?;
    let bytes = std::fs::read(&file).ok()?;
    let cache = serde_json::from_slice::<ScanCache>(&bytes).ok()?;
    if cache.version != SCAN_CACHE_VERSION {
        return None;
    }
    if cache.tree.size == 0 || cache.tree.children.is_empty() {
        return None;
    }
    Some(cache)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_cache_roundtrip_via_json() {
        use crate::knowledge::{Category, Safety};
        let cache = ScanCache {
            version: SCAN_CACHE_VERSION,
            scanned_at: 1234567890,
            root: r"C:\".to_string(),
            tree: TreeNode {
                name: "C:\\".into(),
                path: "C:\\".into(),
                size: 42,
                is_dir: true,
                has_children: false,
                category: Category::Other,
                friendly_name: "x".into(),
                description: "y".into(),
                safety: Safety::Keep,
                children: Vec::new(),
            },
        };
        let json = serde_json::to_string(&cache).unwrap();
        let back: ScanCache = serde_json::from_str(&json).unwrap();
        assert_eq!(back.version, SCAN_CACHE_VERSION);
        assert_eq!(back.scanned_at, 1234567890);
        assert_eq!(back.tree.size, 42);
        assert!(back.tree.children.is_empty());
    }

    /// 旧版缓存（无 version 字段）必须反序列化失败 → 视为未命中重扫。
    #[test]
    fn old_cache_without_version_is_rejected() {
        let old = r#"{"scannedAt":1,"root":"C:\\","tree":{"name":"C:\\","path":"C:\\","size":1,"isDir":true,"hasChildren":false,"category":"other","friendlyName":"x","description":"y","safety":"keep","children":[]}}"#;
        assert!(serde_json::from_str::<ScanCache>(old).is_err());
    }
}
