import { useState, useEffect, useRef } from "react";
import { AnimatePresence } from "framer-motion";
import { listen } from "@tauri-apps/api/event";
import { Zap, FolderInput, ShieldCheck, UploadCloud } from "lucide-react";
import ForceDeleteModal from "../components/ForceDeleteModal";

/**
 * 强力删除工具页：粘贴或拖入一个被占用、删不掉的文件/文件夹路径，
 * 检测占用它的进程 → 确认关闭 → 终止后删除（移入回收站可恢复）。
 */
export default function ForceDelete({ active }: { active: boolean }) {
  const [path, setPath] = useState("");
  const [target, setTarget] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);

  // 拖放事件是整窗口级的，用 ref 保证只在本页可见时响应
  const activeRef = useRef(active);
  activeRef.current = active;

  const trimmed = path.trim().replace(/^"|"$/g, ""); // 容忍“复制为路径”带的引号

  useEffect(() => {
    // Tauri v2 在 Windows 上 webview 级 onDragDropEvent 常不触发，
    // 改监听全局原生事件 tauri://drag-*（官方 issue/社区验证可用）。
    const uns: Promise<() => void>[] = [];
    uns.push(
      listen<{ paths: string[] }>("tauri://drag-drop", (e) => {
        if (!activeRef.current) return;
        setDragging(false);
        const first = e.payload?.paths?.[0];
        if (first) {
          setPath(first);
          setTarget(first); // 拖入即检测占用（仍需在弹窗里确认后才真正删除）
        }
      }),
    );
    uns.push(listen("tauri://drag-enter", () => activeRef.current && setDragging(true)));
    uns.push(listen("tauri://drag-over", () => activeRef.current && setDragging(true)));
    uns.push(listen("tauri://drag-leave", () => setDragging(false)));
    return () => {
      uns.forEach((u) => u.then((f) => f()));
    };
  }, []);

  return (
    <div className="flex h-full flex-col">
      {/* 标题 */}
      <header className="px-8 pt-6 pb-2">
        <h1 className="text-xl font-semibold">强力删除</h1>
        <p className="mt-0.5 text-sm text-[var(--color-text-secondary)]">
          删不掉、提示“文件正在使用”的文件或文件夹，用这里找出占用它的程序，关闭后删除（先进回收站可恢复）
        </p>
      </header>

      {/* 操作区 */}
      <div className="mx-8 mt-2 flex flex-col gap-4 rounded-2xl bg-[var(--color-surface)] p-6 shadow-[var(--shadow-card)]">
        {/* 拖入区 */}
        <div
          className={[
            "flex flex-col items-center justify-center gap-3 rounded-2xl border-2 border-dashed py-20 transition-colors",
            dragging
              ? "border-[var(--color-caution)] bg-[var(--color-caution)]/10"
              : "border-[var(--color-line)] bg-[var(--color-bg)]",
          ].join(" ")}
        >
          <UploadCloud
            size={48}
            className={dragging ? "text-[var(--color-caution)]" : "text-[var(--color-text-secondary)]"}
          />
          <div className="text-base text-[var(--color-text-main)]">
            {dragging ? "松开鼠标，载入该文件 / 文件夹" : "把要删除的文件或文件夹拖到这里"}
          </div>
          <div className="text-xs text-[var(--color-text-secondary)]">或在下方粘贴完整路径</div>
        </div>

        <div>
          <label className="mb-1.5 block text-sm font-medium text-[var(--color-text-main)]">
            文件 / 文件夹路径
          </label>
          <div className="flex items-center gap-2.5">
            <div className="relative flex-1">
              <FolderInput
                size={18}
                className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[var(--color-text-secondary)]"
              />
              <input
                value={path}
                onChange={(e) => setPath(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && trimmed) setTarget(trimmed);
                }}
                placeholder="例如 C:\\Program Files\\顽固软件\\locked.exe"
                className="w-full rounded-xl border border-[var(--color-line)] bg-[var(--color-bg)] py-2.5 pl-10 pr-3 text-sm outline-none focus:border-[var(--color-primary)]"
              />
            </div>
            <button
              onClick={() => trimmed && setTarget(trimmed)}
              disabled={!trimmed}
              className="flex shrink-0 items-center gap-2 rounded-xl bg-[var(--color-caution)] px-4 py-2.5 text-sm font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-40"
            >
              <Zap size={16} />
              检测占用并删除
            </button>
          </div>
        </div>

        {/* 安全说明 */}
        <div className="flex items-start gap-2 rounded-xl border border-[var(--color-line)] px-4 py-3 text-xs leading-relaxed text-[var(--color-text-secondary)]">
          <ShieldCheck size={14} className="mt-0.5 shrink-0 text-[var(--color-safe)]" />
          <div>
            删除默认<b>移入回收站</b>可恢复；系统关键进程（如 lsass、csrss 等）<b>不会</b>被关闭；
            关闭被占用的程序前请先自行保存其中未存的工作。
          </div>
        </div>
      </div>

      <AnimatePresence>
        {target && (
          <ForceDeleteModal
            path={target}
            onClose={() => setTarget(null)}
            onDeleted={() => setPath("")}
          />
        )}
      </AnimatePresence>
    </div>
  );
}
