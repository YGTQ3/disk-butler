import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import {
  Sparkles,
  Loader2,
  ShieldCheck,
  AlertTriangle,
  CheckCircle2,
  XCircle,
  RotateCcw,
  ChevronDown,
  Trash2,
  Gauge,
  Archive,
  Wrench,
  FolderOpen,
  Box,
} from "lucide-react";
import {
  CleanupItem,
  CleanupScan,
  CleanupReport,
  CleanupStats,
  DeepCleanReport,
  DeepAnalyzeReport,
  SystemCleanReport,
  SystemAnalyzeReport,
  formatBytes,
  openInExplorer,
} from "../types";

type Phase = "loading" | "ready" | "confirm" | "cleaning" | "done";
type DeepPhase = "idle" | "intro" | "analyzing" | "analyzed" | "confirm" | "running" | "done";

/** AppData 孤儿残留（已卸载软件遗留目录） */
interface OrphanEntry {
  path: string;
  appName: string;
  size: number;
  /** 非空 = 含云端同步资产等，删前需用户确认（条目下方醒目展示） */
  note: string;
}
interface OrphanScan {
  confirmed: OrphanEntry[];
}

const KIND_META = {
  junk: {
    title: "垃圾残留",
    icon: <Trash2 size={15} />,
    subtitle: (tight: boolean) => (tight ? "删了纯赚，不影响任何功能" : "删了纯赚，不影响任何功能"),
  },
  cache: {
    title: "性能缓存",
    icon: <Gauge size={15} />,
    subtitle: (tight: boolean) =>
      tight
        ? "磁盘空间紧张，已为你勾选可清理的缓存"
        : "缓存能加快软件运行。当前空间充足，建议留着（默认未勾选）",
  },
  data: {
    title: "含个人数据",
    icon: <Archive size={15} />,
    subtitle: () => "删除有实际代价，看清说明再勾选",
  },
} as const;

export default function Cleanup() {
  const [phase, setPhase] = useState<Phase>("loading");
  const [scan, setScan] = useState<CleanupScan | null>(null);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [report, setReport] = useState<CleanupReport | null>(null);
  const [error, setError] = useState("");

  // 高级：系统深度清理（两步式：先分析 → 用户确认 → 再清理）
  const [deepPhase, setDeepPhase] = useState<DeepPhase>("idle");
  const [analyzeReport, setAnalyzeReport] = useState<DeepAnalyzeReport | null>(null);
  const [deepReport, setDeepReport] = useState<DeepCleanReport | null>(null);
  const [deepError, setDeepError] = useState("");
  /** 深度清理完成后的结果弹窗（关闭后卡片内仍保留一行小结） */
  const [showDeepResult, setShowDeepResult] = useState(false);

  // 高级：系统级清理（Windows\Temp + 更新缓存）——引导弹窗 → 分析（只读）→ 结果面板 → 确认弹窗 → 清理 → 完成弹窗
  const [sysPhase, setSysPhase] = useState<
    "idle" | "intro" | "analyzing" | "analyzed" | "confirm" | "running" | "done"
  >("idle");
  const [sysAnalyze, setSysAnalyze] = useState<SystemAnalyzeReport | null>(null);
  const [sysReport, setSysReport] = useState<SystemCleanReport | null>(null);
  const [sysError, setSysError] = useState("");
  const [showSysResult, setShowSysResult] = useState(false);

  async function doSystemAnalyze() {
    setSysPhase("analyzing");
    setSysError("");
    try {
      const r = await invoke<SystemAnalyzeReport>("analyze_system_clean");
      setSysAnalyze(r);
      setSysPhase("analyzed");
    } catch (e) {
      setSysError(String(e));
      setSysPhase("idle");
    }
  }

  async function doSystemClean() {
    setSysPhase("running");
    setSysError("");
    try {
      const r = await invoke<SystemCleanReport>("run_deep_clean_system");
      setSysReport(r);
      setSysPhase("done");
      setShowSysResult(true);
      loadStats();
    } catch (e) {
      setSysError(String(e));
      setSysPhase("analyzed");
    }
  }

  // 累计清理统计（本地持久化的成就感数字）
  const [stats, setStats] = useState<CleanupStats | null>(null);

  // AppData 孤儿残留检查（已卸载软件遗留目录，"回头清存量"）
  const [orphan, setOrphan] = useState<{
    phase: "loading" | "ready" | "confirm";
    report?: OrphanScan;
    checked: Set<string>;
  } | null>(null);
  const [cleaningOrphan, setCleaningOrphan] = useState(false);
  const [orphanMsg, setOrphanMsg] = useState("");

  /** 打开"残留检查"：扫描已卸载软件在 AppData/ProgramData 的遗留目录 */
  async function startOrphanScan() {
    setOrphanMsg("");
    setOrphan({ phase: "loading", checked: new Set() });
    try {
      const report = await invoke<OrphanScan>("scan_orphan_dirs");
      setOrphan({ phase: "ready", report, checked: new Set() });
    } catch (e) {
      setOrphan(null);
      setOrphanMsg(String(e));
    }
  }

  /** 清理勾选的孤儿目录（后端会当场重扫白名单校验，前端选中项不被信任） */
  async function doCleanOrphans() {
    if (!orphan?.report) return;
    setCleaningOrphan(true);
    try {
      const r = await invoke<{ freed: number; errors: string[] }>("clean_orphan_dirs", {
        paths: [...orphan.checked],
      });
      setOrphanMsg(
        r.errors.length > 0 ? `残留清理：${r.errors.join("；")}` : `已清理，释放 ${formatBytes(r.freed)}。`,
      );
      loadStats();
    } catch (e) {
      setOrphanMsg(String(e));
    } finally {
      setCleaningOrphan(false);
      setOrphan(null);
    }
  }

  async function loadStats() {
    try {
      setStats(await invoke<CleanupStats>("get_cleanup_stats"));
    } catch {
      // 统计是锦上添花，失败不打扰用户
    }
  }

  const items = scan?.items ?? [];

  async function load() {
    setPhase("loading");
    setError("");
    setReport(null);
    try {
      const s = await invoke<CleanupScan>("list_cleanup_items");
      setScan(s);
      setChecked(new Set(s.items.filter((i) => i.defaultChecked).map((i) => i.id)));
      setPhase("ready");
    } catch (e) {
      setError(String(e));
      setPhase("ready");
    }
  }

  useEffect(() => {
    load();
    loadStats();
  }, []);

  const selectedSize = useMemo(
    () => items.filter((i) => checked.has(i.id)).reduce((s, i) => s + i.size, 0),
    [items, checked],
  );
  const selectedItems = items.filter((i) => checked.has(i.id));
  const cautionSelected = selectedItems.filter((i) => i.safety === "caution");

  function toggle(id: string) {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  /** 分组全选/全不选 */
  function setGroup(kind: string, on: boolean) {
    setChecked((prev) => {
      const next = new Set(prev);
      for (const i of items.filter((x) => x.kind === kind)) {
        if (on) next.add(i.id);
        else next.delete(i.id);
      }
      return next;
    });
  }

  async function doClean() {
    setPhase("cleaning");
    try {
      const r = await invoke<CleanupReport>("run_cleanup", { ids: [...checked] });
      setReport(r);
      setPhase("done");
      loadStats();
    } catch (e) {
      setError(String(e));
      setPhase("ready");
    }
  }

  async function doDeepAnalyze() {
    setDeepPhase("analyzing");
    setDeepError("");
    try {
      const r = await invoke<DeepAnalyzeReport>("run_deep_analyze");
      setAnalyzeReport(r);
      setDeepPhase("analyzed");
    } catch (e) {
      setDeepError(String(e));
      setDeepPhase("idle");
    }
  }

  async function doDeepClean() {
    setDeepPhase("running");
    setDeepError("");
    try {
      const r = await invoke<DeepCleanReport>("run_deep_clean");
      setDeepReport(r);
      setDeepPhase("done");
      setShowDeepResult(true);
      loadStats();
    } catch (e) {
      setDeepError(String(e));
      setDeepPhase("idle");
    }
  }

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-start justify-between px-8 pt-6 pb-2">
        <div>
          <h1 className="text-xl font-semibold">一键清理</h1>
          <p className="mt-0.5 text-sm text-[var(--color-text-secondary)]">
            只清理经过验证安全的项目，每一项都告诉你“删了会怎样”
          </p>
        </div>
        {(phase === "ready" || phase === "confirm") && (
          <button
            onClick={load}
            title="重新检查各项当前大小"
            className="flex items-center gap-1.5 rounded-xl border border-[var(--color-line)] bg-[var(--color-surface)] px-3.5 py-2 text-xs font-medium text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary-dark)]"
          >
            <RotateCcw size={14} />
            重新检查
          </button>
        )}
      </header>

      {phase === "loading" && (
        <div className="flex flex-1 flex-col items-center justify-center gap-4">
          <Loader2 size={36} className="animate-spin text-[var(--color-primary)]" />
          <div className="text-sm text-[var(--color-text-secondary)]">
            正在计算各项可释放空间…
          </div>
        </div>
      )}

      {phase === "cleaning" && (
        <div className="flex flex-1 flex-col items-center justify-center gap-4">
          <Loader2 size={36} className="animate-spin text-[var(--color-primary)]" />
          <div className="text-sm text-[var(--color-text-secondary)]">
            正在清理选中项目，请稍候…
          </div>
        </div>
      )}

      {phase === "done" && report && (
        <div className="flex flex-1 flex-col items-center gap-6 overflow-y-auto p-8">
          <motion.div
            initial={{ scale: 0.8, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            className="flex h-20 w-20 items-center justify-center rounded-3xl bg-[var(--color-primary-soft)]"
          >
            <CheckCircle2 size={40} className="text-[var(--color-primary)]" />
          </motion.div>
          <div className="text-center">
            <div className="text-2xl font-bold">释放了 {formatBytes(Math.max(0, report.freeAfter - report.freeBefore))}</div>
            <div className="mt-1 text-sm text-[var(--color-text-secondary)]">
              C 盘剩余空间：{formatBytes(report.freeBefore)} →{" "}
              <span className="font-semibold text-[var(--color-primary-dark)]">
                {formatBytes(report.freeAfter)}
              </span>
            </div>
            {stats && stats.totalRuns > 1 && (
              <div className="mt-2 inline-flex items-center gap-1.5 rounded-full bg-[#FEF3C7] px-3.5 py-1.5 text-xs text-[#B45309]">
                <ShieldCheck size={13} />
                历史累计已释放 <b>{formatBytes(stats.totalFreed)}</b>（第 {stats.totalRuns} 次清理）
              </div>
            )}
          </div>
          <div className="w-full max-w-xl rounded-2xl bg-[var(--color-surface)] p-5 shadow-[var(--shadow-card)]">
            {report.results.map((r) => (
              <div
                key={r.id}
                className="flex items-center gap-3 border-b border-[var(--color-line)] py-2.5 text-sm last:border-0"
              >
                {r.error ? (
                  <XCircle size={16} className="shrink-0 text-[var(--color-caution)]" />
                ) : (
                  <CheckCircle2 size={16} className="shrink-0 text-[var(--color-safe)]" />
                )}
                <span className="flex-1">{r.name}</span>
                {r.skipped > 0 && (
                  <span className="text-xs text-[var(--color-text-secondary)]">
                    {r.skipped} 个使用中已跳过
                  </span>
                )}
                <span className="font-medium">{r.error ? r.error : `+${formatBytes(r.freed)}`}</span>
              </div>
            ))}
          </div>
          <button
            onClick={load}
            className="flex items-center gap-2 rounded-xl border border-[var(--color-line)] bg-[var(--color-surface)] px-5 py-2.5 text-sm font-medium transition-colors hover:border-[var(--color-primary)]"
          >
            <RotateCcw size={16} />
            重新检查
          </button>
        </div>
      )}

      {(phase === "ready" || phase === "confirm") && (
        <>
          <div className="flex-1 overflow-y-auto px-8 pb-4 pt-2">
            {error && (
              <div className="mb-4 rounded-xl bg-[var(--color-surface)] px-4 py-3 text-sm text-[var(--color-caution)] shadow-[var(--shadow-card)]">
                {error}
              </div>
            )}

            {/* 累计清理成就：本地统计，金色盾牌荣誉感 */}
            {stats && stats.totalRuns > 0 && (
              <div className="mb-5 flex items-center gap-3.5 rounded-2xl border border-[#FDE68A] bg-gradient-to-r from-[#FFFBEB] to-[var(--color-surface)] px-5 py-3.5 shadow-[var(--shadow-card)]">
                <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-[#FBBF24] to-[#D97706] shadow-[0_2px_8px_rgba(217,119,6,0.35)]">
                  <ShieldCheck size={21} className="text-white" />
                </span>
                <div className="min-w-0">
                  <div className="text-sm">
                    累计已为你释放{" "}
                    <span className="text-lg font-bold text-[#B45309]">
                      {formatBytes(stats.totalFreed)}
                    </span>
                  </div>
                  <div className="text-xs text-[var(--color-text-secondary)]">
                    共清理 {stats.totalRuns} 次 · 每一项都是你亲手确认后才动的
                  </div>
                </div>
              </div>
            )}
            {items.length === 0 && !error && (
              <div className="flex flex-col items-center justify-center gap-3 py-16">
                <ShieldCheck size={36} className="text-[var(--color-primary)]" />
                <div className="text-base font-medium">很干净，没有需要清理的项目</div>
              </div>
            )}

            {(["junk", "cache", "data"] as const).map((kind) => {
              const group = items.filter((i) => i.kind === kind);
              if (group.length === 0) return null;
              const meta = KIND_META[kind];
              const allOn = group.every((i) => checked.has(i.id));
              return (
                <div key={kind} className="mb-6">
                  <div className="mb-2 flex items-center gap-2">
                    <span className="flex items-center gap-1.5 text-sm font-semibold">
                      {meta.icon}
                      {meta.title}
                    </span>
                    <span className="text-xs text-[var(--color-text-secondary)]">
                      {meta.subtitle(scan?.spaceTight ?? false)}
                    </span>
                    {/* 分组快捷全选/全不选 */}
                    <button
                      onClick={() => setGroup(kind, !allOn)}
                      className="ml-auto rounded-lg border border-[var(--color-line)] px-2.5 py-1 text-[11px] text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary-dark)]"
                    >
                      {allOn ? "全部取消" : "全部选中"}
                    </button>
                  </div>
                  <div className="space-y-2.5">
                    {group.map((i) => (
                      <ItemCard key={i.id} item={i} checked={checked.has(i.id)} onToggle={toggle} />
                    ))}
                  </div>
                </div>
              );
            })}

            {/* 高级 · 系统深度清理 */}
            <div className="mb-6">
              <div className="mb-2 flex items-center gap-2">
                <span className="flex items-center gap-1.5 text-sm font-semibold">
                  <Wrench size={15} />
                  高级 · 系统深度清理
                </span>
                <span className="text-xs text-[var(--color-text-secondary)]">
                  收益大但有代价，看懂说明再用
                </span>
              </div>
              <div className="rounded-2xl bg-[var(--color-surface)] p-4 shadow-[var(--shadow-card)]">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-semibold">清理 Windows 更新旧版本备份 (WinSxS)</span>
                  <span className="rounded-full bg-[#FEE2E2] px-2 py-0.5 text-[10px] font-medium text-[#991B1B]">
                    需要管理员
                  </span>
                  <span className="rounded-full bg-[var(--color-bg)] px-2 py-0.5 text-[10px] text-[var(--color-text-secondary)]">
                    约 5~20 分钟
                  </span>
                </div>
                <div className="mt-1 text-xs leading-relaxed text-[var(--color-text-secondary)]">
                  系统每次更新都会保留旧组件用于回滚，日积月累可达数 GB～十几 GB。本操作调用微软官方
                  DISM 工具清理这些备份，与系统自带磁盘清理同源，不会损坏系统。
                </div>
                <div className="mt-0.5 text-xs leading-relaxed">
                  <span className="text-[var(--color-text-secondary)]">删了会怎样：</span>
                  ⚠ 清理后将无法卸载/回滚现有的 Windows 更新（系统运行正常则基本无影响）。
                </div>

                {/* 分析中 */}
                {deepPhase === "analyzing" && (
                  <div className="mt-3 flex items-center gap-2 rounded-xl bg-[var(--color-bg)] px-3.5 py-2.5 text-xs text-[var(--color-text-secondary)]">
                    <Loader2 size={14} className="animate-spin text-[var(--color-primary)]" />
                    正在分析中（只读，不做任何更改）……约 1~3 分钟。黑色终端窗口会自己关闭，请不要手动关它。
                  </div>
                )}

                {(deepPhase === "analyzed" || deepPhase === "confirm") && analyzeReport && (
                  <div className="mt-3 flex gap-3">
                    {/* 左：微软原始报告（专业信息保留） */}
                    <div className="min-w-0 flex-1 rounded-xl bg-[var(--color-bg)] p-3.5">
                      <div className="mb-1.5 text-xs font-medium">📊 微软 DISM 原始报告</div>
                      <div className="select-text space-y-0.5 text-[11px] leading-relaxed text-[var(--color-text-secondary)]">
                        {analyzeReport.lines.map((l, idx) => (
                          <div key={idx}>{l}</div>
                        ))}
                      </div>
                    </div>
                    {/* 右：人话重点标注（大字） */}
                    <div className="flex w-56 shrink-0 flex-col items-center justify-center gap-3 rounded-xl bg-[var(--color-primary-soft)] p-4 text-center">
                      {analyzeReport.backupGb !== null ? (
                        <div>
                          <div className="text-xs text-[var(--color-text-secondary)]">预计可释放</div>
                          <div className="mt-0.5 text-2xl font-bold text-[var(--color-primary-dark)]">
                            约 {(analyzeReport.backupGb * 0.5).toFixed(1)}~
                            {(analyzeReport.backupGb * 0.7).toFixed(1)} GB
                          </div>
                        </div>
                      ) : (
                        <div className="text-sm text-[var(--color-text-secondary)]">
                          未能识别可释放量，请看左侧报告
                        </div>
                      )}
                      {analyzeReport.recommended === true && (
                        <div className="rounded-full bg-[var(--color-safe)] px-4 py-1.5 text-sm font-semibold text-white">
                          微软推荐清理 ✓
                        </div>
                      )}
                      {analyzeReport.recommended === false && (
                        <div className="rounded-full bg-[var(--color-keep)] px-4 py-1.5 text-sm font-semibold text-white">
                          暂不必清理
                        </div>
                      )}
                      <button
                        onClick={() => setDeepPhase("confirm")}
                        className="w-full rounded-xl bg-[var(--color-primary)] py-2.5 text-sm font-semibold text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                      >
                        确认清理…
                      </button>
                    </div>
                  </div>
                )}

                {deepPhase === "running" && (
                  <div className="mt-3 flex items-center gap-2 rounded-xl bg-[var(--color-bg)] px-3.5 py-2.5 text-xs text-[var(--color-text-secondary)]">
                    <Loader2 size={14} className="animate-spin text-[var(--color-primary)]" />
                    正在清理系统组件（约 5~20 分钟）……进度窗口会自己关闭，请不要手动关它，也请勿关机。
                  </div>
                )}
                {deepPhase === "done" && deepReport && (
                  <div className="mt-3 rounded-xl bg-[var(--color-primary-soft)] px-3.5 py-2.5 text-xs">
                    ✅ 深度清理完成，释放{" "}
                    <b>{formatBytes(deepReport.freed)}</b>（C 盘剩余{" "}
                    {formatBytes(deepReport.freeBefore)} → {formatBytes(deepReport.freeAfter)}）
                  </div>
                )}
                {deepError && (
                  <div className="mt-3 rounded-xl bg-[#FEF3C7] px-3.5 py-2.5 text-xs text-[#92400E]">
                    {deepError}
                  </div>
                )}

                {(deepPhase === "idle" || deepPhase === "analyzed") && (
                  <button
                    onClick={() => setDeepPhase("intro")}
                    className="mt-3 rounded-xl border border-[var(--color-line)] px-4 py-2 text-xs font-medium transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary-dark)]"
                  >
                    {deepPhase === "analyzed" ? "重新分析" : "分析（只读）"}
                  </button>
                )}
              </div>
            </div>

            {/* 高级 · 系统临时文件 + 更新缓存（单步提权） */}
            <div className="mb-6">
              <div className="mb-2 flex items-center gap-2">
                <span className="flex items-center gap-1.5 text-sm font-semibold">
                  <Wrench size={15} />
                  高级 · 系统临时文件 + 更新缓存
                </span>
                <span className="text-xs text-[var(--color-text-secondary)]">
                  普通清理够不到的系统级垃圾
                </span>
              </div>
              <div className="rounded-2xl bg-[var(--color-surface)] p-4 shadow-[var(--shadow-card)]">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-semibold">清理 Windows 系统临时文件 + 更新下载缓存</span>
                  <span className="rounded-full bg-[#FEE2E2] px-2 py-0.5 text-[10px] font-medium text-[#991B1B]">
                    需要管理员
                  </span>
                </div>
                <div className="mt-1 text-xs leading-relaxed text-[var(--color-text-secondary)]">
                  清理 <code>C:\Windows\Temp</code>（系统临时文件，受权限保护、普通清理够不到）和 Windows
                  更新下载缓存（<code>SoftwareDistribution\Download</code>，装完更新残留的安装包）。
                </div>
                <div className="mt-0.5 text-xs leading-relaxed">
                  <span className="text-[var(--color-text-secondary)]">删了会怎样：</span>
                  没有影响，都会按需自动重建；正在使用的文件自动跳过。若检测到有挂起的 Windows
                  更新，会自动只清临时文件、跳过更新缓存，避免打断更新。
                </div>

                {/* 分析中（只读，需一次授权量出系统 Temp 大小） */}
                {sysPhase === "analyzing" && (
                  <div className="mt-3 flex items-center gap-2 rounded-xl bg-[var(--color-bg)] px-3.5 py-2.5 text-xs text-[var(--color-text-secondary)]">
                    <Loader2 size={14} className="animate-spin text-[var(--color-primary)]" />
                    正在分析中（只读，不删任何东西）……需要一次管理员授权来量出系统临时目录大小。
                  </div>
                )}

                {/* 分析结果：左明细 + 右绿色底「预计可释放」大字 + 清理按钮（与系统深度清理同款） */}
                {sysPhase === "analyzed" && sysAnalyze && (
                  <div className="mt-3 flex gap-3">
                    {/* 左：各项当前大小明细（数字加粗更黑，一眼看清） */}
                    <div className="min-w-0 flex-1 rounded-xl bg-[var(--color-bg)] p-3.5">
                      <div className="mb-2 text-xs font-medium">📊 各项当前大小</div>
                      <div className="space-y-1.5 text-xs text-[var(--color-text-secondary)]">
                        <div className="flex items-center justify-between gap-2">
                          <span>系统临时文件（Windows\Temp）</span>
                          <b className="shrink-0 text-sm text-[var(--color-text-main)]">
                            {formatBytes(sysAnalyze.tempBytes)}
                          </b>
                        </div>
                        <div className="flex items-center justify-between gap-2">
                          <span>更新下载缓存（SoftwareDistribution\Download）</span>
                          <b className="shrink-0 text-sm text-[var(--color-text-main)]">
                            {formatBytes(sysAnalyze.updateCacheBytes)}
                          </b>
                        </div>
                        {sysAnalyze.updatePending && (
                          <div className="text-[var(--color-keep)]">· 检测到挂起更新，清理时将跳过更新缓存</div>
                        )}
                      </div>
                    </div>
                    {/* 右：绿色底预计释放 + 清理按钮 */}
                    <div className="flex w-56 shrink-0 flex-col items-center justify-center gap-3 rounded-xl bg-[var(--color-primary-soft)] p-4 text-center">
                      <div>
                        <div className="text-xs text-[var(--color-text-secondary)]">预计可释放</div>
                        <div className="mt-0.5 text-2xl font-bold text-[var(--color-primary-dark)]">
                          {formatBytes(
                            sysAnalyze.tempBytes +
                              (sysAnalyze.updatePending ? 0 : sysAnalyze.updateCacheBytes)
                          )}
                        </div>
                      </div>
                      {sysAnalyze.tempBytes +
                        (sysAnalyze.updatePending ? 0 : sysAnalyze.updateCacheBytes) <
                      50 * 1024 * 1024 ? (
                        <div className="text-xs text-[var(--color-text-secondary)]">
                          目前不多，可以先不清、等攒多了再来
                        </div>
                      ) : null}
                      <button
                        onClick={() => setSysPhase("confirm")}
                        className="w-full rounded-xl bg-[var(--color-primary)] py-2.5 text-sm font-semibold text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                      >
                        确认清理…
                      </button>
                    </div>
                  </div>
                )}

                {sysPhase === "running" && (
                  <div className="mt-3 flex items-center gap-2 rounded-xl bg-[var(--color-bg)] px-3.5 py-2.5 text-xs text-[var(--color-text-secondary)]">
                    <Loader2 size={14} className="animate-spin text-[var(--color-primary)]" />
                    正在清理系统临时文件与更新缓存……授权后稍等片刻，进度窗口会自己关闭，请不要手动关它。
                  </div>
                )}
                {sysPhase === "done" && sysReport && (
                  <div className="mt-3 rounded-xl bg-[var(--color-primary-soft)] px-3.5 py-2.5 text-xs">
                    ✅ 系统清理完成，释放 <b>{formatBytes(sysReport.freed)}</b>（C 盘剩余{" "}
                    {formatBytes(sysReport.freeBefore)} → {formatBytes(sysReport.freeAfter)}）
                    {sysReport.updateCacheSkipped && (
                      <span className="mt-1 block text-[var(--color-text-secondary)]">
                        注：检测到有挂起的 Windows 更新，已跳过更新缓存、只清了临时文件。
                      </span>
                    )}
                  </div>
                )}
                {sysError && (
                  <div className="mt-3 rounded-xl bg-[#FEF3C7] px-3.5 py-2.5 text-xs text-[#92400E]">
                    {sysError}
                  </div>
                )}

                {(sysPhase === "idle" || sysPhase === "analyzed" || sysPhase === "done") && (
                  <button
                    onClick={() => setSysPhase("intro")}
                    className="mt-3 rounded-xl border border-[var(--color-line)] px-4 py-2 text-xs font-medium transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary-dark)]"
                  >
                    {sysPhase === "idle" ? "分析（只读，需管理员）" : "重新分析"}
                  </button>
                )}
              </div>
            </div>

            {/* 高级 · 残留检查（已卸载软件遗留目录） */}
            <div className="mb-6">
              <div className="mb-2 flex items-center gap-2">
                <span className="flex items-center gap-1.5 text-sm font-semibold">
                  <Box size={15} />
                  高级 · 残留检查
                </span>
                <span className="text-xs text-[var(--color-text-secondary)]">
                  找出已卸载软件留在电脑上的文件夹
                </span>
              </div>
              <div className="rounded-2xl bg-[var(--color-surface)] p-4 shadow-[var(--shadow-card)]">
                <div className="text-xs leading-relaxed text-[var(--color-text-secondary)]">
                  有些软件卸载后，会在 AppData/ProgramData 里留下文件夹没删干净，日积月累占空间。这里帮你比对"已安装软件清单"，**只列出确认是已卸载软件遗留的文件夹**——拿不准归属的一律不列，避免误伤在用的软件。
                </div>

                {/* 扫描中 */}
                {orphan?.phase === "loading" && (
                  <div className="mt-3 flex items-center gap-2 rounded-xl bg-[var(--color-bg)] px-3.5 py-2.5 text-xs text-[var(--color-text-secondary)]">
                    <Loader2 size={14} className="animate-spin text-[var(--color-primary)]" />
                    正在比对已装软件与 AppData 目录…
                  </div>
                )}

                {/* 扫描结果：内联展示可勾选清单（与其他高级功能一致，不再用弹窗） */}
                {(orphan?.phase === "ready" || orphan?.phase === "confirm") && orphan.report && (
                  <div className="mt-3">
                    {orphan.report.confirmed.length === 0 ? (
                      <div className="rounded-xl bg-[var(--color-bg)] px-3.5 py-6 text-center text-sm text-[var(--color-text-secondary)]">
                        没有发现已卸载软件的遗留文件夹，很干净 👍
                      </div>
                    ) : (
                      <>
                        <div className="mb-2 text-xs leading-relaxed text-[var(--color-text-secondary)]">
                          这些文件夹的主人已被卸载、文件夹还留着。勾选后可清理；删前可先「打开位置」看看有没有想留的。
                        </div>
                        <ul className="space-y-1.5">
                          {orphan.report.confirmed.map((o) => {
                            const on = orphan.checked.has(o.path);
                            return (
                              <li key={o.path}>
                                <label className="flex cursor-pointer items-start gap-2.5 rounded-xl bg-[var(--color-bg)] px-3.5 py-2.5">
                                  <input
                                    type="checkbox"
                                    checked={on}
                                    onChange={() =>
                                      setOrphan((cur) => {
                                        if (!cur) return cur;
                                        const s = new Set(cur.checked);
                                        on ? s.delete(o.path) : s.add(o.path);
                                        return { ...cur, checked: s, phase: "ready" };
                                      })
                                    }
                                    className="mt-0.5"
                                  />
                                  <span className="min-w-0 flex-1">
                                    <span className="text-[13px] font-medium text-[var(--color-text-main)]">
                                      {o.appName}（已卸载）· {formatBytes(o.size)}
                                    </span>
                                    <span className="block break-all text-xs text-[var(--color-text-secondary)]">{o.path}</span>
                                    {o.note && (
                                      <span className="mt-1.5 flex items-start gap-1.5 rounded-lg bg-[#FFFBEB] px-2.5 py-1.5 text-xs leading-relaxed text-[#92400E]">
                                        <AlertTriangle size={13} className="mt-0.5 shrink-0" />
                                        {o.note}
                                      </span>
                                    )}
                                  </span>
                                  <button
                                    onClick={(ev) => {
                                      ev.preventDefault();
                                      openInExplorer(o.path, true);
                                    }}
                                    className="shrink-0 rounded-lg border border-[var(--color-line)] px-2 py-1 text-xs text-[var(--color-text-secondary)] hover:bg-[var(--color-surface)]"
                                  >
                                    打开位置
                                  </button>
                                </label>
                              </li>
                            );
                          })}
                        </ul>
                        {orphan.phase === "confirm" && (
                          <div className="mt-3 rounded-xl bg-[#FEF2F2] px-4 py-3 text-sm leading-relaxed text-[#991B1B]">
                            将永久删除勾选的 {orphan.checked.size} 个文件夹（不进回收站）。确定里面没有要留的东西了吗？
                          </div>
                        )}
                      </>
                    )}
                  </div>
                )}

                {orphanMsg && (
                  <div className="mt-3 rounded-xl bg-[var(--color-primary-soft)] px-3.5 py-2.5 text-xs">{orphanMsg}</div>
                )}

                {/* 操作按钮：检查/重新检查 + 清理所选 + 确认删除 */}
                <div className="mt-3 flex flex-wrap gap-2">
                  {(!orphan || orphan.phase === "ready") && (
                    <button
                      onClick={startOrphanScan}
                      disabled={cleaningOrphan}
                      className="flex items-center gap-1.5 rounded-xl border border-[var(--color-line)] px-4 py-2 text-xs font-medium transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary-dark)] disabled:opacity-50"
                    >
                      <Box size={13} />
                      {orphan?.report ? "重新检查" : "开始检查（只读）"}
                    </button>
                  )}
                  {orphan?.phase === "ready" && orphan.report && orphan.report.confirmed.length > 0 && (
                    <button
                      disabled={orphan.checked.size === 0}
                      onClick={() => setOrphan({ ...orphan, phase: "confirm" })}
                      className="rounded-xl bg-[var(--color-primary)] px-4 py-2 text-xs font-semibold text-white transition-colors hover:bg-[var(--color-primary-dark)] disabled:opacity-50"
                    >
                      清理所选（{orphan.checked.size}）
                    </button>
                  )}
                  {orphan?.phase === "confirm" && (
                    <>
                      <button
                        disabled={cleaningOrphan}
                        onClick={() => setOrphan({ ...orphan, phase: "ready" })}
                        className="rounded-xl border border-[var(--color-line)] px-4 py-2 text-xs font-medium transition-colors hover:bg-[var(--color-bg)] disabled:opacity-50"
                      >
                        再想想
                      </button>
                      <button
                        disabled={cleaningOrphan || orphan.checked.size === 0}
                        onClick={doCleanOrphans}
                        className="rounded-xl bg-[#DC2626] px-4 py-2 text-xs font-semibold text-white transition-colors hover:bg-[#B91C1C] disabled:opacity-50"
                      >
                        {cleaningOrphan ? "清理中…" : "确认删除"}
                      </button>
                    </>
                  )}
                </div>
              </div>
            </div>
          </div>

          {/* 底部操作条 */}
          {items.length > 0 && (
            <div className="border-t border-[var(--color-line)] bg-[var(--color-surface)] px-8 py-4">
              <div className="flex items-center justify-between">
                <div className="text-sm text-[var(--color-text-secondary)]">
                  已选 {selectedItems.length} 项 · 预计可释放{" "}
                  <span className="text-lg font-bold text-[var(--color-primary-dark)]">
                    {formatBytes(selectedSize)}
                  </span>
                </div>
                <button
                  disabled={selectedItems.length === 0}
                  onClick={() => setPhase("confirm")}
                  className="flex items-center gap-2 rounded-xl bg-[var(--color-primary)] px-6 py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)] disabled:opacity-40"
                >
                  <Sparkles size={17} />
                  清理选中项
                </button>
              </div>
            </div>
          )}

          {/* 常规清理确认弹窗 */}
          <AnimatePresence>
            {phase === "confirm" && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="absolute inset-0 z-30 flex items-center justify-center bg-black/30 p-6"
                onClick={() => setPhase("ready")}
              >
                <motion.div
                  initial={{ scale: 0.94, y: 10 }}
                  animate={{ scale: 1, y: 0 }}
                  exit={{ scale: 0.94, y: 10 }}
                  className="w-full max-w-lg rounded-2xl bg-[var(--color-surface)] p-6 shadow-[var(--shadow-card-hover)]"
                  onClick={(e) => e.stopPropagation()}
                >
                  <div className="text-base font-semibold">清理前，最后核对一遍</div>
                  <div className="mt-1 text-sm text-[var(--color-text-secondary)]">
                    将清理以下 {selectedItems.length} 项，共约{" "}
                    <b className="text-[var(--color-primary-dark)]">{formatBytes(selectedSize)}</b>
                    。每一项都标注了“清了会怎样”，请扫一眼再确认：
                  </div>

                  {/* 完整清单：谨慎项排前醒目，每项卡片化 = 名称 + 大小 + 影响说明 */}
                  <div className="mt-4 max-h-72 space-y-2 overflow-y-auto pr-1">
                    {[...cautionSelected, ...selectedItems.filter((i) => i.safety !== "caution")].map(
                      (i) => (
                        <div
                          key={i.id}
                          className={[
                            "rounded-xl border p-3",
                            i.safety === "caution"
                              ? "border-[#FDE68A] bg-[#FFFBEB]"
                              : "border-[var(--color-line)] bg-[var(--color-bg)]",
                          ].join(" ")}
                        >
                          <div className="flex items-center gap-2">
                            {i.safety === "caution" ? (
                              <AlertTriangle size={14} className="shrink-0 text-[#D97706]" />
                            ) : (
                              <CheckCircle2 size={14} className="shrink-0 text-[var(--color-primary)]" />
                            )}
                            <span className="text-sm font-medium">{i.name}</span>
                            <span className="ml-auto shrink-0 text-xs font-bold text-[var(--color-primary-dark)]">
                              {formatBytes(i.size)}
                            </span>
                          </div>
                          <div
                            className={[
                              "mt-1 pl-6 text-xs leading-relaxed",
                              i.safety === "caution"
                                ? "text-[#92400E]"
                                : "text-[var(--color-text-secondary)]",
                            ].join(" ")}
                          >
                            {i.impact}
                          </div>
                        </div>
                      )
                    )}
                  </div>

                  <div className="mt-3 text-[11px] text-[var(--color-text-secondary)]">
                    只删除列出位置里的内容（目录本身保留）；正在使用中的文件会自动跳过。
                  </div>

                  <div className="mt-4 flex gap-3">
                    <button
                      onClick={() => setPhase("ready")}
                      className="flex-1 rounded-xl border border-[var(--color-line)] py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]"
                    >
                      再想想
                    </button>
                    <button
                      onClick={doClean}
                      className="flex-1 rounded-xl bg-[var(--color-primary)] py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                    >
                      确认清理这 {selectedItems.length} 项
                    </button>
                  </div>
                </motion.div>
              </motion.div>
            )}
          </AnimatePresence>

          {/* 分析前预告弹窗：先告知接下来会发生什么，确认后才开始 */}
          <AnimatePresence>
            {deepPhase === "intro" && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="absolute inset-0 z-30 flex items-center justify-center bg-black/30 p-6"
                onClick={() => setDeepPhase(analyzeReport ? "analyzed" : "idle")}
              >
                <motion.div
                  initial={{ scale: 0.94, y: 10 }}
                  animate={{ scale: 1, y: 0 }}
                  exit={{ scale: 0.94, y: 10 }}
                  className="w-full max-w-lg rounded-2xl bg-[var(--color-surface)] p-7 shadow-[var(--shadow-card-hover)]"
                  onClick={(e) => e.stopPropagation()}
                >
                  <div className="text-lg font-semibold">开始分析前，先看这 3 件事</div>
                  <div className="mt-1 text-xs text-[var(--color-text-secondary)]">
                    这一步只是“体检”，用来告诉你能清理多少
                  </div>

                  <div className="mt-5 space-y-3">
                    <div className="flex items-center gap-3.5 rounded-xl bg-[var(--color-bg)] px-4 py-3">
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-primary)] text-sm font-bold text-white">
                        1
                      </span>
                      <span className="min-w-0 text-sm">
                        <span className="font-semibold">弹出系统授权窗口（UAC），请点“是”</span>
                        <span className="mt-0.5 block text-xs text-[var(--color-text-secondary)]">
                          这是 Windows 对管理员操作的正常确认
                        </span>
                      </span>
                    </div>
                    <div className="flex items-center gap-3.5 rounded-xl bg-[var(--color-bg)] px-4 py-3">
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-primary)] text-sm font-bold text-white">
                        2
                      </span>
                      <span className="min-w-0 text-sm">
                        <span className="font-semibold">会出现黑色终端窗口，它会自己关闭</span>
                        <span className="mt-0.5 block text-xs text-[var(--color-text-secondary)]">
                          那是分析进度窗口，请不要手动关它
                        </span>
                      </span>
                    </div>
                    <div className="flex items-center gap-3.5 rounded-xl bg-[var(--color-bg)] px-4 py-3">
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-primary)] text-sm font-bold text-white">
                        3
                      </span>
                      <span className="min-w-0 text-sm">
                        <span className="font-semibold">全程约 1~3 分钟</span>
                        <span className="mt-0.5 block text-xs text-[var(--color-text-secondary)]">
                          期间可以正常使用电脑
                        </span>
                      </span>
                    </div>
                  </div>

                  <div className="mt-4 flex items-center gap-2.5 rounded-xl bg-[var(--color-primary-soft)] px-4 py-3">
                    <ShieldCheck size={18} className="shrink-0 text-[var(--color-primary-dark)]" />
                    <span className="text-sm">
                      <b>只读取信息，不删除、不修改任何东西</b>
                      <span className="text-[var(--color-text-secondary)]">，分析完由你决定要不要清理</span>
                    </span>
                  </div>

                  <div className="mt-6 flex gap-3">
                    <button
                      onClick={() => setDeepPhase(analyzeReport ? "analyzed" : "idle")}
                      className="flex-1 rounded-xl border border-[var(--color-line)] py-3 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]"
                    >
                      取消
                    </button>
                    <button
                      onClick={doDeepAnalyze}
                      className="flex-1 rounded-xl bg-[var(--color-primary)] py-3 text-sm font-semibold text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                    >
                      确定开始
                    </button>
                  </div>
                </motion.div>
              </motion.div>
            )}
          </AnimatePresence>

          {/* 深度清理确认弹窗 */}
          <AnimatePresence>
            {deepPhase === "confirm" && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="absolute inset-0 z-30 flex items-center justify-center bg-black/30 p-6"
                onClick={() => setDeepPhase("analyzed")}
              >
                <motion.div
                  initial={{ scale: 0.94, y: 10 }}
                  animate={{ scale: 1, y: 0 }}
                  exit={{ scale: 0.94, y: 10 }}
                  className="w-full max-w-lg rounded-2xl bg-[var(--color-surface)] p-7 shadow-[var(--shadow-card-hover)]"
                  onClick={(e) => e.stopPropagation()}
                >
                  <div className="flex items-center gap-3">
                    <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-[#FEE2E2]">
                      <AlertTriangle size={24} className="text-[#DC2626]" />
                    </span>
                    <span>
                      <span className="block text-lg font-semibold">确认执行系统深度清理？</span>
                      <span className="block text-xs text-[var(--color-text-secondary)]">
                        开始前请看清下面 3 件事
                      </span>
                    </span>
                  </div>

                  <div className="mt-5 space-y-3">
                    <div className="flex items-center gap-3.5 rounded-xl bg-[var(--color-bg)] px-4 py-3">
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-primary)] text-sm font-bold text-white">
                        1
                      </span>
                      <span className="min-w-0 text-sm">
                        <span className="font-semibold">弹出系统授权窗口（UAC），请点“是”</span>
                        <span className="mt-0.5 block text-xs text-[var(--color-text-secondary)]">
                          之后会出现进度窗口，它会自己关闭，请不要手动关它
                        </span>
                      </span>
                    </div>
                    <div className="flex items-center gap-3.5 rounded-xl border border-[#FECACA] bg-[#FEF2F2] px-4 py-3.5">
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[#DC2626] text-sm font-bold text-white">
                        !
                      </span>
                      <span className="min-w-0">
                        <span className="block text-base font-bold text-[#B91C1C]">
                          全程约 5~20 分钟，期间请勿关机
                        </span>
                        <span className="mt-0.5 block text-xs text-[#B91C1C] opacity-80">
                          中途断电可能损坏系统组件，这是唯一需要你保证的事
                        </span>
                      </span>
                    </div>
                    <div className="flex items-center gap-3.5 rounded-xl bg-[var(--color-bg)] px-4 py-3">
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-keep)] text-sm font-bold text-white">
                        3
                      </span>
                      <span className="min-w-0 text-sm">
                        <span className="font-semibold">清理后无法卸载/回滚现有的 Windows 更新</span>
                        <span className="mt-0.5 block text-xs text-[var(--color-text-secondary)]">
                          系统运行正常的话，这基本没有影响
                        </span>
                      </span>
                    </div>
                  </div>

                  <div className="mt-6 flex gap-3">
                    <button
                      onClick={() => setDeepPhase("analyzed")}
                      className="flex-1 rounded-xl border border-[var(--color-line)] py-3 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]"
                    >
                      再想想
                    </button>
                    <button
                      onClick={doDeepClean}
                      className="flex-1 rounded-xl bg-[#DC2626] py-3 text-sm font-semibold text-white transition-colors hover:bg-[#B91C1C]"
                    >
                      我已了解，开始清理
                    </button>
                  </div>
                </motion.div>
              </motion.div>
            )}
          </AnimatePresence>

          {/* 深度清理完成：结果弹窗（大字庆祝，替代原先不起眼的一行小字） */}
          <AnimatePresence>
            {deepPhase === "done" && deepReport && showDeepResult && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="absolute inset-0 z-30 flex items-center justify-center bg-black/30 p-6"
                onClick={() => setShowDeepResult(false)}
              >
                <motion.div
                  initial={{ scale: 0.9, y: 14 }}
                  animate={{ scale: 1, y: 0 }}
                  exit={{ scale: 0.94, y: 10 }}
                  className="w-full max-w-lg rounded-2xl bg-[var(--color-surface)] p-8 text-center shadow-[var(--shadow-card-hover)]"
                  onClick={(e) => e.stopPropagation()}
                >
                  <motion.div
                    initial={{ scale: 0.6, opacity: 0 }}
                    animate={{ scale: 1, opacity: 1 }}
                    transition={{ delay: 0.1, type: "spring", stiffness: 260 }}
                    className="mx-auto flex h-20 w-20 items-center justify-center rounded-3xl bg-[var(--color-primary-soft)]"
                  >
                    <CheckCircle2 size={44} className="text-[var(--color-primary)]" />
                  </motion.div>

                  <div className="mt-4 text-lg font-semibold">系统深度清理完成 🎉</div>
                  <div className="mt-3 text-xs text-[var(--color-text-secondary)]">本次释放</div>
                  <div className="text-4xl font-bold text-[var(--color-primary-dark)]">
                    {formatBytes(deepReport.freed)}
                  </div>
                  <div className="mt-2 text-sm text-[var(--color-text-secondary)]">
                    C 盘剩余空间：{formatBytes(deepReport.freeBefore)} →{" "}
                    <span className="font-semibold text-[var(--color-primary-dark)]">
                      {formatBytes(deepReport.freeAfter)}
                    </span>
                  </div>

                  {deepReport.freed < 300 * 1024 * 1024 && (
                    <div className="mt-4 rounded-xl bg-[var(--color-bg)] px-4 py-3 text-left text-xs leading-relaxed text-[var(--color-text-secondary)]">
                      💡 本次释放不多，通常是因为距离上次清理不久，系统还没积累多少旧备份——这是正常现象。等
                      Windows 更新攒了几个月，这里一次能清出几个 GB。
                    </div>
                  )}

                  {stats && stats.totalRuns > 0 && (
                    <div className="mt-4 inline-flex items-center gap-1.5 rounded-full bg-[#FEF3C7] px-4 py-1.5 text-xs text-[#B45309]">
                      <ShieldCheck size={13} />
                      历史累计已释放 <b>{formatBytes(stats.totalFreed)}</b>
                    </div>
                  )}

                  <button
                    onClick={() => setShowDeepResult(false)}
                    className="mt-6 w-full rounded-xl bg-[var(--color-primary)] py-3 text-sm font-semibold text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                  >
                    好的，收下了
                  </button>
                </motion.div>
              </motion.div>
            )}
          </AnimatePresence>

          {/* 系统清理·分析前引导弹窗 */}
          <AnimatePresence>
            {sysPhase === "intro" && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="absolute inset-0 z-30 flex items-center justify-center bg-black/30 p-6"
                onClick={() => setSysPhase(sysAnalyze ? "analyzed" : "idle")}
              >
                <motion.div
                  initial={{ scale: 0.94, y: 10 }}
                  animate={{ scale: 1, y: 0 }}
                  exit={{ scale: 0.94, y: 10 }}
                  className="w-full max-w-lg rounded-2xl bg-[var(--color-surface)] p-7 shadow-[var(--shadow-card-hover)]"
                  onClick={(e) => e.stopPropagation()}
                >
                  <div className="text-lg font-semibold">开始分析前，先看这 3 件事</div>
                  <div className="mt-1 text-xs text-[var(--color-text-secondary)]">
                    这一步只是“体检”，用来告诉你能清理多少
                  </div>

                  <div className="mt-5 space-y-3">
                    <div className="flex items-center gap-3.5 rounded-xl bg-[var(--color-bg)] px-4 py-3">
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-primary)] text-sm font-bold text-white">
                        1
                      </span>
                      <span className="min-w-0 text-sm">
                        <span className="font-semibold">弹出系统授权窗口（UAC），请点“是”</span>
                        <span className="mt-0.5 block text-xs text-[var(--color-text-secondary)]">
                          系统临时目录受权限保护，需要管理员才能量出真实大小
                        </span>
                      </span>
                    </div>
                    <div className="flex items-center gap-3.5 rounded-xl bg-[var(--color-bg)] px-4 py-3">
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-primary)] text-sm font-bold text-white">
                        2
                      </span>
                      <span className="min-w-0 text-sm">
                        <span className="font-semibold">会出现黑色终端窗口，它会自己关闭</span>
                        <span className="mt-0.5 block text-xs text-[var(--color-text-secondary)]">
                          那是量大小的窗口，请不要手动关它
                        </span>
                      </span>
                    </div>
                    <div className="flex items-center gap-3.5 rounded-xl bg-[var(--color-bg)] px-4 py-3">
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-primary)] text-sm font-bold text-white">
                        3
                      </span>
                      <span className="min-w-0 text-sm">
                        <span className="font-semibold">很快，通常几秒到十几秒</span>
                        <span className="mt-0.5 block text-xs text-[var(--color-text-secondary)]">
                          期间可以正常使用电脑
                        </span>
                      </span>
                    </div>
                  </div>

                  <div className="mt-4 flex items-center gap-2.5 rounded-xl bg-[var(--color-primary-soft)] px-4 py-3">
                    <ShieldCheck size={18} className="shrink-0 text-[var(--color-primary-dark)]" />
                    <span className="text-sm">
                      <b>只读取信息，不删除、不修改任何东西</b>
                      <span className="text-[var(--color-text-secondary)]">，分析完由你决定要不要清理</span>
                    </span>
                  </div>

                  <div className="mt-6 flex gap-3">
                    <button
                      onClick={() => setSysPhase(sysAnalyze ? "analyzed" : "idle")}
                      className="flex-1 rounded-xl border border-[var(--color-line)] py-3 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]"
                    >
                      取消
                    </button>
                    <button
                      onClick={doSystemAnalyze}
                      className="flex-1 rounded-xl bg-[var(--color-primary)] py-3 text-sm font-semibold text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                    >
                      确定开始
                    </button>
                  </div>
                </motion.div>
              </motion.div>
            )}
          </AnimatePresence>

          {/* 系统清理·确认清理弹窗 */}
          <AnimatePresence>
            {sysPhase === "confirm" && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="absolute inset-0 z-30 flex items-center justify-center bg-black/30 p-6"
                onClick={() => setSysPhase("analyzed")}
              >
                <motion.div
                  initial={{ scale: 0.94, y: 10 }}
                  animate={{ scale: 1, y: 0 }}
                  exit={{ scale: 0.94, y: 10 }}
                  className="w-full max-w-lg rounded-2xl bg-[var(--color-surface)] p-7 shadow-[var(--shadow-card-hover)]"
                  onClick={(e) => e.stopPropagation()}
                >
                  <div className="flex items-center gap-3">
                    <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-[var(--color-primary-soft)]">
                      <Wrench size={22} className="text-[var(--color-primary-dark)]" />
                    </span>
                    <span>
                      <span className="block text-lg font-semibold">确认清理系统临时文件 + 更新缓存？</span>
                      <span className="block text-xs text-[var(--color-text-secondary)]">
                        预计可释放{" "}
                        <b className="text-[var(--color-primary-dark)]">
                          {sysAnalyze
                            ? formatBytes(
                                sysAnalyze.tempBytes +
                                  (sysAnalyze.updatePending ? 0 : sysAnalyze.updateCacheBytes)
                              )
                            : ""}
                        </b>
                      </span>
                    </span>
                  </div>

                  <div className="mt-5 space-y-3">
                    <div className="flex items-center gap-3.5 rounded-xl bg-[var(--color-bg)] px-4 py-3">
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-primary)] text-sm font-bold text-white">
                        1
                      </span>
                      <span className="min-w-0 text-sm">
                        <span className="font-semibold">弹出系统授权窗口（UAC），请点“是”</span>
                        <span className="mt-0.5 block text-xs text-[var(--color-text-secondary)]">
                          之后会出现进度窗口，它会自己关闭，请不要手动关它
                        </span>
                      </span>
                    </div>
                    <div className="flex items-center gap-3.5 rounded-xl bg-[var(--color-bg)] px-4 py-3">
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-primary)] text-sm font-bold text-white">
                        2
                      </span>
                      <span className="min-w-0 text-sm">
                        <span className="font-semibold">会短暂停止 Windows 更新服务，清完自动恢复</span>
                        <span className="mt-0.5 block text-xs text-[var(--color-text-secondary)]">
                          用于清理更新下载缓存；有挂起更新时会自动跳过这步、只清临时文件
                        </span>
                      </span>
                    </div>
                    <div className="flex items-center gap-3.5 rounded-xl bg-[var(--color-bg)] px-4 py-3">
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-safe)] text-sm font-bold text-white">
                        ✓
                      </span>
                      <span className="min-w-0 text-sm">
                        <span className="font-semibold">没有影响，都会按需自动重建</span>
                        <span className="mt-0.5 block text-xs text-[var(--color-text-secondary)]">
                          正在使用的文件自动跳过；很快，通常几秒到几十秒
                        </span>
                      </span>
                    </div>
                  </div>

                  <div className="mt-6 flex gap-3">
                    <button
                      onClick={() => setSysPhase("analyzed")}
                      className="flex-1 rounded-xl border border-[var(--color-line)] py-3 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]"
                    >
                      再想想
                    </button>
                    <button
                      onClick={doSystemClean}
                      className="flex-1 rounded-xl bg-[var(--color-primary)] py-3 text-sm font-semibold text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                    >
                      我已了解，开始清理
                    </button>
                  </div>
                </motion.div>
              </motion.div>
            )}
          </AnimatePresence>

          {/* 系统清理·完成庆祝弹窗 */}
          <AnimatePresence>
            {sysPhase === "done" && sysReport && showSysResult && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="absolute inset-0 z-30 flex items-center justify-center bg-black/30 p-6"
                onClick={() => setShowSysResult(false)}
              >
                <motion.div
                  initial={{ scale: 0.9, y: 14 }}
                  animate={{ scale: 1, y: 0 }}
                  exit={{ scale: 0.94, y: 10 }}
                  className="w-full max-w-lg rounded-2xl bg-[var(--color-surface)] p-8 text-center shadow-[var(--shadow-card-hover)]"
                  onClick={(e) => e.stopPropagation()}
                >
                  <motion.div
                    initial={{ scale: 0.6, opacity: 0 }}
                    animate={{ scale: 1, opacity: 1 }}
                    transition={{ delay: 0.1, type: "spring", stiffness: 260 }}
                    className="mx-auto flex h-20 w-20 items-center justify-center rounded-3xl bg-[var(--color-primary-soft)]"
                  >
                    <CheckCircle2 size={44} className="text-[var(--color-primary)]" />
                  </motion.div>

                  <div className="mt-4 text-lg font-semibold">系统清理完成 🎉</div>
                  <div className="mt-3 text-xs text-[var(--color-text-secondary)]">本次释放</div>
                  <div className="text-4xl font-bold text-[var(--color-primary-dark)]">
                    {formatBytes(sysReport.freed)}
                  </div>
                  <div className="mt-2 text-sm text-[var(--color-text-secondary)]">
                    C 盘剩余空间：{formatBytes(sysReport.freeBefore)} →{" "}
                    <span className="font-semibold text-[var(--color-primary-dark)]">
                      {formatBytes(sysReport.freeAfter)}
                    </span>
                  </div>

                  {sysReport.updateCacheSkipped && (
                    <div className="mt-4 rounded-xl bg-[var(--color-bg)] px-4 py-3 text-left text-xs leading-relaxed text-[var(--color-text-secondary)]">
                      💡 检测到有挂起的 Windows 更新，本次已跳过更新缓存、只清了系统临时文件，避免打断更新。等更新装完再清可释放更多。
                    </div>
                  )}

                  {stats && stats.totalRuns > 0 && (
                    <div className="mt-4 inline-flex items-center gap-1.5 rounded-full bg-[#FEF3C7] px-4 py-1.5 text-xs text-[#B45309]">
                      <ShieldCheck size={13} />
                      历史累计已释放 <b>{formatBytes(stats.totalFreed)}</b>
                    </div>
                  )}

                  <button
                    onClick={() => setShowSysResult(false)}
                    className="mt-6 w-full rounded-xl bg-[var(--color-primary)] py-3 text-sm font-semibold text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                  >
                    好的，收下了
                  </button>
                </motion.div>
              </motion.div>
            )}
          </AnimatePresence>

        </>
      )}
    </div>
  );
}

function ItemCard({
  item,
  checked,
  onToggle,
}: {
  item: CleanupItem;
  checked: boolean;
  onToggle: (id: string) => void;
}) {
  const caution = item.safety === "caution";
  const [expanded, setExpanded] = useState(false);
  return (
    <div
      className={[
        "rounded-2xl bg-[var(--color-surface)] p-4 shadow-[var(--shadow-card)] transition-shadow hover:shadow-[var(--shadow-card-hover)]",
        checked ? "ring-2 ring-[var(--color-primary)]" : "",
      ].join(" ")}
    >
      <label className="flex cursor-pointer items-start gap-3.5">
        <input
          type="checkbox"
          checked={checked}
          onChange={() => onToggle(item.id)}
          className="mt-1 h-4 w-4 accent-[var(--color-primary)]"
        />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-sm font-semibold">{item.name}</span>
            {caution && (
              <span className="rounded-full bg-[#FEF3C7] px-2 py-0.5 text-[10px] font-medium text-[#92400E]">
                谨慎
              </span>
            )}
            {item.negligible && (
              <span
                className="rounded-full bg-[var(--color-bg)] px-2 py-0.5 text-[10px] text-[var(--color-text-secondary)]"
                title="占用不到剩余空间的 1%，删了也感觉不到，可以不管它"
              >
                占用很小，可忽略
              </span>
            )}
            <span className="ml-auto text-sm font-bold text-[var(--color-primary-dark)]">
              {formatBytes(item.size)}
            </span>
          </div>
          <div className="mt-1 text-xs leading-relaxed text-[var(--color-text-secondary)]">
            {item.description}
          </div>
          <div className="mt-0.5 text-xs leading-relaxed">
            <span className="text-[var(--color-text-secondary)]">删了会怎样：</span>
            {item.impact}
          </div>
        </div>
      </label>

      <button
        onClick={(e) => {
          e.preventDefault();
          setExpanded((v) => !v);
        }}
        className="mt-2 ml-7 flex items-center gap-1 text-xs font-medium text-[var(--color-primary-dark)] transition-colors hover:text-[var(--color-primary)]"
      >
        <ChevronDown
          size={13}
          className={["transition-transform", expanded ? "rotate-180" : ""].join(" ")}
        />
        {expanded ? "收起详情" : `查看详情（将清理 ${item.paths.length} 个位置）`}
      </button>
      <AnimatePresence>
        {expanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.2, ease: "easeOut" }}
            className="ml-7 overflow-hidden"
          >
            <div className="mt-2 select-text rounded-xl bg-[var(--color-bg)] p-3">
              <div className="mb-1.5 text-[11px] text-[var(--color-text-secondary)]">
                只删除以下位置里的内容（目录本身保留），不会碰其它任何文件：
              </div>
              <div className="max-h-44 space-y-1 overflow-y-auto">
                {item.paths.map((p) => (
                  <div
                    key={p.path}
                    className="group flex items-baseline justify-between gap-3 text-[11px] leading-relaxed"
                  >
                    <span className="break-all text-[var(--color-text-secondary)]">{p.path}</span>
                    <span className="ml-auto shrink-0 font-medium">{formatBytes(p.size)}</span>
                    <button
                      onClick={() =>
                        item.id === "recycle-bin"
                          ? void invoke("open_recycle_bin").catch((e) =>
                              alert(`没能打开回收站：${String(e)}`),
                            )
                          : void openInExplorer(p.path, true)
                      }
                      title={
                        item.id === "recycle-bin"
                          ? "打开回收站，亲眼看看里面是什么"
                          : "在文件夹中打开，亲眼看看里面是什么"
                      }
                      className="shrink-0 self-center rounded-md p-1 text-[var(--color-text-secondary)] opacity-60 transition-all hover:bg-[var(--color-surface)] hover:text-[var(--color-primary-dark)] hover:opacity-100"
                    >
                      <FolderOpen size={13} />
                    </button>
                  </div>
                ))}
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
