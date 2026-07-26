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
} from "lucide-react";
import {
  CleanupItem,
  CleanupScan,
  CleanupReport,
  CleanupStats,
  DeepCleanReport,
  DeepAnalyzeReport,
  formatBytes,
} from "../types";

type Phase = "loading" | "ready" | "confirm" | "cleaning" | "done";
type DeepPhase = "idle" | "intro" | "analyzing" | "analyzed" | "confirm" | "running" | "done";

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

  // 累计清理统计（本地持久化的成就感数字）
  const [stats, setStats] = useState<CleanupStats | null>(null);

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
            <div className="text-2xl font-bold">释放了 {formatBytes(report.totalFreed)}</div>
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
                  className="w-full max-w-md rounded-2xl bg-[var(--color-surface)] p-6 shadow-[var(--shadow-card-hover)]"
                  onClick={(e) => e.stopPropagation()}
                >
                  <div className="text-base font-semibold">
                    确认清理这 {selectedItems.length} 项？
                  </div>
                  <div className="mt-1 text-sm text-[var(--color-text-secondary)]">
                    共 {formatBytes(selectedSize)}。正在使用中的文件会自动跳过。
                  </div>
                  {cautionSelected.length > 0 && (
                    <div className="mt-4 rounded-xl bg-[#FEF3C7] p-3.5">
                      <div className="flex items-center gap-1.5 text-sm font-medium text-[#92400E]">
                        <AlertTriangle size={15} />
                        以下项目有代价，请再次确认
                      </div>
                      <ul className="mt-1.5 space-y-1 text-xs leading-relaxed text-[#92400E]">
                        {cautionSelected.map((i) => (
                          <li key={i.id}>
                            <b>{i.name}</b>：{i.impact}
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}
                  <div className="mt-5 flex gap-3">
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
                      确认清理
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
                    className="flex items-baseline justify-between gap-3 text-[11px] leading-relaxed"
                  >
                    <span className="break-all text-[var(--color-text-secondary)]">{p.path}</span>
                    <span className="shrink-0 font-medium">{formatBytes(p.size)}</span>
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
