/** 与 Rust 端 serde 序列化结构一一对应的类型定义 */

export type Category =
  | "system"
  | "software"
  | "cache"
  | "personal"
  | "systemFile"
  | "other";

export type Safety = "safe" | "caution" | "keep";

export interface TreeNode {
  name: string;
  path: string;
  size: number;
  isDir: boolean;
  hasChildren: boolean;
  category: Category;
  friendlyName: string;
  description: string;
  safety: Safety;
  children?: TreeNode[];
}

export interface ScanProgress {
  filesScanned: number;
  bytesScanned: number;
  currentPath: string;
  done: boolean;
}

export interface DriveInfo {
  letter: string;
  mountPoint: string;
  total: number;
  free: number;
  used: number;
}

export interface PathDetail {
  path: string;
  size: number;
}

export interface CleanupItem {
  id: string;
  name: string;
  description: string;
  impact: string;
  paths: PathDetail[];
  size: number;
  safety: "safe" | "caution";
  kind: "junk" | "cache" | "data";
  negligible: boolean;
  defaultChecked: boolean;
}

export interface CleanupScan {
  items: CleanupItem[];
  free: number;
  spaceTight: boolean;
}

export interface DeepCleanReport {
  freed: number;
  freeBefore: number;
  freeAfter: number;
}

/** 累计清理统计（本地持久化） */
export interface CleanupStats {
  totalFreed: number;
  totalRuns: number;
  lastAt: number;
}

/** 页面文件一致性核验 */
export interface PagefileEntry {
  path: string;
  drive: string;
  initMb: number;
  maxMb: number;
  systemManaged: boolean;
}

export interface ActivePagefile {
  path: string;
  drive: string;
  allocatedMb: number;
}

export interface PagefileCheck {
  autoManaged: boolean;
  configured: PagefileEntry[];
  active: ActivePagefile[];
  /** 为空 = 一切正常 */
  issues: string[];
}

export interface DeepAnalyzeReport {
  lines: string[];
  recommended: boolean | null;
  backupGb: number | null;
}

export interface ItemResult {
  id: string;
  name: string;
  freed: number;
  skipped: number;
  error: string | null;
}

export interface CleanupReport {
  results: ItemResult[];
  totalFreed: number;
  freeBefore: number;
  freeAfter: number;
}

export interface StartupItem {
  id: string;
  name: string;
  command: string;
  location: string;
  enabled: boolean;
  needsAdmin: boolean;
  memMb: number;
  advice: "disable" | "keep" | "neutral";
  reason: string;
}

export interface MemoryOverview {
  total: number;
  used: number;
  available: number;
  swapTotal: number;
  swapUsed: number;
}

export interface ProcessGroup {
  name: string;
  friendlyName: string;
  description: string;
  kind: "closable" | "system" | "unknown";
  count: number;
  memory: number;
}

export interface MemoryReport {
  overview: MemoryOverview;
  groups: ProcessGroup[];
}

export interface ScanCache {
  scannedAt: number;
  root: string;
  tree: TreeNode;
}

/** 分类 -> 设计令牌色（与 tokens.css / DESIGN.md 一致） */
export const CATEGORY_COLOR: Record<Category, string> = {
  system: "var(--color-cat-system)",
  software: "var(--color-cat-software)",
  cache: "var(--color-cat-cache)",
  personal: "var(--color-cat-personal)",
  systemFile: "var(--color-cat-system-file)",
  other: "var(--color-cat-other)",
};

export const CATEGORY_LABEL: Record<Category, string> = {
  system: "系统",
  software: "软件",
  cache: "缓存",
  personal: "个人文件",
  systemFile: "系统文件",
  other: "未识别",
};

export const SAFETY_META: Record<Safety, { label: string; color: string }> = {
  safe: { label: "可安全清理", color: "var(--color-safe)" },
  caution: { label: "谨慎操作", color: "var(--color-caution)" },
  keep: { label: "请勿删除", color: "var(--color-keep)" },
};

/** 人性化容量显示：1610612736 -> "1.5 GB" */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(Math.floor(Math.log2(bytes) / 10), units.length - 1);
  const value = bytes / 2 ** (10 * i);
  return `${value >= 100 ? value.toFixed(0) : value.toFixed(1)} ${units[i]}`;
}

/** 人性化数字：12345678 -> "1234.6 万" */
export function formatCount(n: number): string {
  if (n >= 10000) return `${(n / 10000).toFixed(1)} 万`;
  return n.toLocaleString();
}

/** 相对时间：Unix 秒 -> "刚刚 / 5 分钟前 / 3 小时前 / 2 天前" */
export function formatAgo(unixSecs: number): string {
  const diff = Math.max(0, Date.now() / 1000 - unixSecs);
  if (diff < 60) return "刚刚";
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`;
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`;
  return `${Math.floor(diff / 86400)} 天前`;
}
