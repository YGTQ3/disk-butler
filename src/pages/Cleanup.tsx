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
} from "lucide-react";
import { CleanupItem, CleanupReport, formatBytes } from "../types";

type Phase = "loading" | "ready" | "confirm" | "cleaning" | "done";

export default function Cleanup() {
  const [phase, setPhase] = useState<Phase>("loading");
  const [items, setItems] = useState<CleanupItem[]>([]);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [report, setReport] = useState<CleanupReport | null>(null);
  const [error, setError] = useState("");

  async function load() {
    setPhase("loading");
    setError("");
    setReport(null);
    try {
      const list = await invoke<CleanupItem[]>("list_cleanup_items");
      setItems(list);
      setChecked(new Set(list.filter((i) => i.defaultChecked).map((i) => i.id)));
      setPhase("ready");
    } catch (e) {
      setError(String(e));
      setPhase("ready");
    }
  }

  useEffect(() => {
    load();
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

  async function doClean() {
    setPhase("cleaning");
    try {
      const r = await invoke<CleanupReport>("run_cleanup", {
        ids: [...checked],
      });
      setReport(r);
      setPhase("done");
    } catch (e) {
      setError(String(e));
      setPhase("ready");
    }
  }

  const safeItems = items.filter((i) => i.safety === "safe");
  const cautionItems = items.filter((i) => i.safety === "caution");

  return (
    <div className="flex h-full flex-col">
      <header className="px-8 pt-6 pb-2">
        <h1 className="text-xl font-semibold">一键清理</h1>
        <p className="mt-0.5 text-sm text-[var(--color-text-secondary)]">
          只清理经过验证安全的项目，每一项都告诉你"删了会怎样"
        </p>
      </header>

      {/* 加载中 */}
      {phase === "loading" && (
        <div className="flex flex-1 flex-col items-center justify-center gap-4">
          <Loader2 size={36} className="animate-spin text-[var(--color-primary)]" />
          <div className="text-sm text-[var(--color-text-secondary)]">
            正在计算各项可释放空间…
          </div>
        </div>
      )}

      {/* 清理中 */}
      {phase === "cleaning" && (
        <div className="flex flex-1 flex-col items-center justify-center gap-4">
          <Loader2 size={36} className="animate-spin text-[var(--color-primary)]" />
          <div className="text-sm text-[var(--color-text-secondary)]">
            正在清理选中项目，请稍候…
          </div>
        </div>
      )}

      {/* 清理完成：结果对比 */}
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
            <div className="text-2xl font-bold">
              释放了 {formatBytes(report.totalFreed)}
            </div>
            <div className="mt-1 text-sm text-[var(--color-text-secondary)]">
              C 盘剩余空间：{formatBytes(report.freeBefore)} → {" "}
              <span className="font-semibold text-[var(--color-primary-dark)]">
                {formatBytes(report.freeAfter)}
              </span>
            </div>
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
                <span className="font-medium">
                  {r.error ? r.error : `+${formatBytes(r.freed)}`}
                </span>
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

      {/* 列表 */}
      {(phase === "ready" || phase === "confirm") && (
        <>
          <div className="flex-1 overflow-y-auto px-8 pb-4 pt-2">
            {error && (
              <div className="mb-4 rounded-xl bg-[var(--color-surface)] px-4 py-3 text-sm text-[var(--color-caution)] shadow-[var(--shadow-card)]">
                {error}
              </div>
            )}
            {items.length === 0 && !error && (
              <div className="flex h-full flex-col items-center justify-center gap-3">
                <ShieldCheck size={36} className="text-[var(--color-primary)]" />
                <div className="text-base font-medium">很干净，没有需要清理的项目</div>
              </div>
            )}

            {safeItems.length > 0 && (
              <Section title="放心清理" subtitle="删除后没有任何影响，缓存会按需重新生成">
                {safeItems.map((i) => (
                  <ItemCard key={i.id} item={i} checked={checked.has(i.id)} onToggle={toggle} />
                ))}
              </Section>
            )}
            {cautionItems.length > 0 && (
              <Section
                title="谨慎选择"
                subtitle="有一定代价，看清说明后再勾选（默认不勾选）"
              >
                {cautionItems.map((i) => (
                  <ItemCard key={i.id} item={i} checked={checked.has(i.id)} onToggle={toggle} />
                ))}
              </Section>
            )}
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

          {/* 确认弹窗 */}
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
                  <div className="text-base font-semibold">确认清理这 {selectedItems.length} 项？</div>
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
        </>
      )}
    </div>
  );
}

function Section({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle: string;
  children: React.ReactNode;
}) {
  return (
    <div className="mb-6">
      <div className="mb-2 flex items-baseline gap-2">
        <span className="text-sm font-semibold">{title}</span>
        <span className="text-xs text-[var(--color-text-secondary)]">{subtitle}</span>
      </div>
      <div className="space-y-2.5">{children}</div>
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

      {/* 详情展开：具体会清理哪些路径 */}
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
