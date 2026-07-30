import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { Loader2, ShieldAlert, Zap, CheckCircle2, AlertTriangle } from "lucide-react";
import { LockingProcess } from "../types";

/**
 * 强力删除弹窗：检测占用某文件/文件夹的进程 → 让用户确认关闭 → 终止后删除（移入回收站可恢复）。
 * 供「独立强力删除页」与「软件卸载·残留清理」复用。
 */
export default function ForceDeleteModal({
  path,
  onClose,
  onDeleted,
}: {
  path: string;
  onClose: () => void;
  onDeleted?: (path: string) => void;
}) {
  const [loading, setLoading] = useState(true);
  const [procs, setProcs] = useState<LockingProcess[]>([]);
  const [checked, setChecked] = useState<Set<number>>(new Set());
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const [done, setDone] = useState(false);

  useEffect(() => {
    let alive = true;
    invoke<LockingProcess[]>("find_file_lockers", { path })
      .then((list) => {
        if (!alive) return;
        setProcs(list);
        // 默认勾选所有“非系统关键”进程
        setChecked(new Set(list.filter((p) => !p.isCritical).map((p) => p.pid)));
      })
      .catch((e) => alive && setError(String(e)))
      .finally(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
  }, [path]);

  function toggle(pid: number) {
    setChecked((prev) => {
      const next = new Set(prev);
      next.has(pid) ? next.delete(pid) : next.add(pid);
      return next;
    });
  }

  async function run() {
    setWorking(true);
    setError("");
    try {
      const pids = Array.from(checked);
      await invoke("force_delete_path", { path, pids, toRecycle: true });
      setDone(true);
      onDeleted?.(path);
    } catch (e) {
      setError(String(e));
    } finally {
      setWorking(false);
    }
  }

  const killable = procs.filter((p) => !p.isCritical);

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6"
      onClick={onClose}
    >
      <motion.div
        initial={{ scale: 0.96, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        exit={{ scale: 0.96, opacity: 0 }}
        onClick={(e) => e.stopPropagation()}
        className="flex max-h-[80vh] w-[520px] flex-col overflow-hidden rounded-2xl bg-[var(--color-surface)] shadow-xl"
      >
        {/* 头部 */}
        <div className="flex items-start gap-3 border-b border-[var(--color-line)] px-6 py-4">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-[var(--color-caution)]/15 text-[var(--color-caution)]">
            <Zap size={18} />
          </div>
          <div className="min-w-0 flex-1">
            <div className="text-base font-semibold">强力删除</div>
            <div className="mt-0.5 truncate text-xs text-[var(--color-text-secondary)]" title={path}>
              {path}
            </div>
          </div>
        </div>

        {/* 主体 */}
        <div className="flex-1 overflow-y-auto px-6 py-4">
          {loading ? (
            <div className="flex items-center gap-2 py-6 text-sm text-[var(--color-text-secondary)]">
              <Loader2 size={16} className="animate-spin" />
              正在检测占用进程…
            </div>
          ) : done ? (
            <div className="flex items-center gap-2 py-6 text-sm text-[var(--color-safe)]">
              <CheckCircle2 size={18} />
              已删除（文件已移入回收站，可恢复）
            </div>
          ) : procs.length === 0 ? (
            <div className="py-2 text-sm text-[var(--color-text-main)]">
              没有检测到占用进程。删除失败可能是权限或其他原因，可直接尝试强力删除（受保护位置会请求一次管理员授权）。
            </div>
          ) : (
            <>
              <div className="mb-3 text-sm text-[var(--color-text-main)]">
                以下程序正占用它，关闭后即可删除：
              </div>
              <div className="flex flex-col gap-1.5">
                {procs.map((p) => (
                  <label
                    key={p.pid}
                    className={[
                      "flex items-center gap-2.5 rounded-lg border px-3 py-2",
                      p.isCritical
                        ? "cursor-not-allowed border-[var(--color-line)] bg-[var(--color-bg)] opacity-60"
                        : "cursor-pointer border-[var(--color-line)] bg-[var(--color-bg)]",
                    ].join(" ")}
                  >
                    <input
                      type="checkbox"
                      disabled={p.isCritical}
                      checked={checked.has(p.pid)}
                      onChange={() => toggle(p.pid)}
                      className="h-4 w-4 shrink-0 accent-[var(--color-primary)]"
                    />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm text-[var(--color-text-main)]">{p.name}</div>
                      <div className="text-[11px] text-[var(--color-text-secondary)]">PID {p.pid}</div>
                    </div>
                    {p.isCritical && (
                      <span className="flex shrink-0 items-center gap-1 text-[11px] text-[var(--color-caution)]">
                        <ShieldAlert size={12} />
                        系统关键进程，不可关闭
                      </span>
                    )}
                  </label>
                ))}
              </div>
            </>
          )}

          {error && (
            <div className="mt-3 flex items-start gap-1.5 rounded-lg bg-[var(--color-caution)]/10 px-3 py-2 text-xs text-[var(--color-caution)]">
              <AlertTriangle size={13} className="mt-0.5 shrink-0" />
              <span className="min-w-0 flex-1">{error}</span>
            </div>
          )}
        </div>

        {/* 底部按钮 */}
        <div className="flex items-center justify-end gap-2 border-t border-[var(--color-line)] px-6 py-3">
          <button
            onClick={onClose}
            className="rounded-xl px-4 py-2 text-sm text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg)]"
          >
            {done ? "关闭" : "取消"}
          </button>
          {!done && !loading && (
            <button
              onClick={run}
              disabled={working}
              className="flex items-center gap-2 rounded-xl bg-[var(--color-caution)] px-4 py-2 text-sm font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-40"
            >
              {working ? (
                <Loader2 size={15} className="animate-spin" />
              ) : (
                <Zap size={15} />
              )}
              {killable.length > 0 ? `关闭并删除 (${checked.size})` : "直接强力删除"}
            </button>
          )}
        </div>
      </motion.div>
    </motion.div>
  );
}
