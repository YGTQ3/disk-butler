import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, RotateCcw, Cpu, MemoryStick } from "lucide-react";
import { MemoryReport, formatBytes } from "../types";

type Phase = "loading" | "ready";

/** 内存水位 -> 人话状态 */
function healthLabel(percent: number): { text: string; color: string } {
  if (percent < 60) return { text: "健康", color: "var(--color-safe)" };
  if (percent < 80) return { text: "偏高", color: "var(--color-caution)" };
  return { text: "紧张", color: "#EF4444" };
}

const KIND_BADGE = {
  system: { text: "系统", bg: "#DBEAFE", fg: "#1E40AF" },
  closable: { text: "可退出", bg: "#D1FAE5", fg: "#065F46" },
  unknown: { text: "未识别", bg: "#F1F5F9", fg: "#475569" },
} as const;

export default function MemoryCheck() {
  const [phase, setPhase] = useState<Phase>("loading");
  const [report, setReport] = useState<MemoryReport | null>(null);
  const [error, setError] = useState("");

  async function load() {
    setPhase("loading");
    setError("");
    try {
      setReport(await invoke<MemoryReport>("memory_report"));
    } catch (e) {
      setError(String(e));
    }
    setPhase("ready");
  }

  useEffect(() => {
    load();
  }, []);

  const ov = report?.overview;
  const memPercent = ov ? (ov.used / ov.total) * 100 : 0;
  const swapPercent = ov && ov.swapTotal > 0 ? (ov.swapUsed / ov.swapTotal) * 100 : 0;
  const health = healthLabel(memPercent);
  const closableTotal =
    report?.groups.filter((g) => g.kind === "closable").reduce((s, g) => s + g.memory, 0) ?? 0;

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-start justify-between px-8 pt-6 pb-2">
        <div>
          <h1 className="text-xl font-semibold">内存体检</h1>
          <p className="mt-0.5 text-sm text-[var(--color-text-secondary)]">
            看看是谁在占内存，哪些可以随手放掉
          </p>
        </div>
        {phase === "ready" && (
          <button
            onClick={load}
            className="flex items-center gap-1.5 rounded-xl border border-[var(--color-line)] bg-[var(--color-surface)] px-3.5 py-2 text-xs font-medium text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary-dark)]"
          >
            <RotateCcw size={14} />
            重新体检
          </button>
        )}
      </header>

      {phase === "loading" && (
        <div className="flex flex-1 flex-col items-center justify-center gap-4">
          <Loader2 size={36} className="animate-spin text-[var(--color-primary)]" />
          <div className="text-sm text-[var(--color-text-secondary)]">正在采集内存数据…</div>
        </div>
      )}

      {phase === "ready" && (
        <div className="flex-1 overflow-y-auto px-8 pb-6 pt-2">
          {error && (
            <div className="mb-4 rounded-xl bg-[var(--color-surface)] px-4 py-3 text-sm text-[var(--color-caution)] shadow-[var(--shadow-card)]">
              {error}
            </div>
          )}

          {ov && (
            <div className="mb-5 grid grid-cols-2 gap-4">
              {/* 物理内存 */}
              <div className="rounded-2xl bg-[var(--color-surface)] p-5 shadow-[var(--shadow-card)]">
                <div className="flex items-center gap-2 text-sm font-semibold">
                  <MemoryStick size={16} className="text-[var(--color-primary)]" />
                  物理内存
                  <span
                    className="ml-auto rounded-full px-2.5 py-0.5 text-xs font-medium text-white"
                    style={{ background: health.color }}
                  >
                    {health.text} {memPercent.toFixed(0)}%
                  </span>
                </div>
                <div className="mt-3 h-3 w-full overflow-hidden rounded-full bg-[var(--color-line)]">
                  <div
                    className="h-full rounded-full transition-all"
                    style={{ width: `${memPercent}%`, background: health.color }}
                  />
                </div>
                <div className="mt-2 text-xs text-[var(--color-text-secondary)]">
                  已用 {formatBytes(ov.used)} / 共 {formatBytes(ov.total)}，可用{" "}
                  <b className="text-[var(--color-text-main)]">{formatBytes(ov.available)}</b>
                </div>
              </div>

              {/* 页面文件 */}
              <div className="rounded-2xl bg-[var(--color-surface)] p-5 shadow-[var(--shadow-card)]">
                <div className="flex items-center gap-2 text-sm font-semibold">
                  <Cpu size={16} className="text-[var(--color-primary)]" />
                  页面文件（虚拟内存）
                  <span className="ml-auto text-xs font-medium text-[var(--color-text-secondary)]">
                    {swapPercent.toFixed(0)}%
                  </span>
                </div>
                <div className="mt-3 h-3 w-full overflow-hidden rounded-full bg-[var(--color-line)]">
                  <div
                    className="h-full rounded-full bg-[var(--color-cat-system-file)] transition-all"
                    style={{ width: `${swapPercent}%` }}
                  />
                </div>
                <div className="mt-2 text-xs text-[var(--color-text-secondary)]">
                  已用 {formatBytes(ov.swapUsed)} / 共 {formatBytes(ov.swapTotal)}
                  。用得多说明物理内存不够，系统在借硬盘周转
                </div>
              </div>
            </div>
          )}

          {/* 人话结论 */}
          {ov && (
            <div className="mb-5 rounded-2xl bg-[var(--color-primary-soft)] px-5 py-3.5 text-sm leading-relaxed">
              {memPercent < 60 &&
                "内存状态良好，无需特别处理。"}
              {memPercent >= 60 && memPercent < 80 && (
                <>
                  内存水位偏高。下面标着「可退出」的应用合计占了{" "}
                  <b>{formatBytes(closableTotal)}</b>
                  ，暂时不用的可以退出释放。
                </>
              )}
              {memPercent >= 80 && (
                <>
                  内存很紧张！建议立刻退出暂时不用的「可退出」应用（合计{" "}
                  <b>{formatBytes(closableTotal)}</b>
                  ）；长期建议考虑升级物理内存。
                </>
              )}
            </div>
          )}

          {/* 大户排行 */}
          <div className="mb-2 text-sm font-semibold">内存占用排行（按程序分组）</div>
          <div className="space-y-2">
            {report?.groups.map((g) => {
              const badge = KIND_BADGE[g.kind];
              const barPercent = ov ? (g.memory / ov.total) * 100 : 0;
              return (
                <div
                  key={g.name}
                  className="rounded-2xl bg-[var(--color-surface)] px-4 py-3 shadow-[var(--shadow-card)]"
                >
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-semibold">{g.friendlyName}</span>
                    {g.friendlyName !== g.name && (
                      <span className="text-xs text-[var(--color-text-secondary)]">{g.name}</span>
                    )}
                    <span
                      className="rounded-full px-2 py-0.5 text-[10px] font-medium"
                      style={{ background: badge.bg, color: badge.fg }}
                    >
                      {badge.text}
                    </span>
                    {g.count > 1 && (
                      <span className="text-[11px] text-[var(--color-text-secondary)]">
                        ×{g.count} 进程
                      </span>
                    )}
                    <span className="ml-auto text-sm font-bold text-[var(--color-primary-dark)]">
                      {formatBytes(g.memory)}
                    </span>
                  </div>
                  <div className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-[var(--color-bg)]">
                    <div
                      className="h-full rounded-full bg-[var(--color-primary)] opacity-60"
                      style={{ width: `${Math.max(barPercent, 0.5)}%` }}
                    />
                  </div>
                  <div className="mt-1 text-xs leading-relaxed text-[var(--color-text-secondary)]">
                    {g.description}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
