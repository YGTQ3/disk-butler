import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import {
  Trash2,
  Loader2,
  Search,
  Package,
  RotateCcw,
  FolderOpen,
  KeyRound,
  ShieldCheck,
  AlertTriangle,
  CheckCircle2,
  XCircle,
} from "lucide-react";
import { InstalledApp, LeftoverItem, RemoveResult, formatBytes } from "../types";

type Phase = "loading" | "ready";

/** 残留清单弹窗的数据：来源软件名 + 合并后的残留项 */
interface LeftoverView {
  appNames: string[];
  items: LeftoverItem[];
}

const leftoverKey = (it: { kind: string; path: string }) => `${it.kind}::${it.path}`;

export default function Uninstall() {
  const [phase, setPhase] = useState<Phase>("loading");
  const [apps, setApps] = useState<InstalledApp[]>([]);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  /** 批量选中的软件 id */
  const [selected, setSelected] = useState<Set<string>>(new Set());

  /** 卸载器执行中的遮罩（等待原生向导完成）*/
  const [busy, setBusy] = useState<string>("");

  /** 残留清单弹窗 */
  const [leftover, setLeftover] = useState<LeftoverView | null>(null);
  const [leftoverChecked, setLeftoverChecked] = useState<Set<string>>(new Set());
  const [removing, setRemoving] = useState(false);
  const [removeResults, setRemoveResults] = useState<RemoveResult[] | null>(null);

  /** 图标缓存：按 app.id（回退会用到 installLocation，不能只按 iconPath 缓存） */
  const [icons, setIcons] = useState<Record<string, string>>({});
  const requestedIcons = useRef<Set<string>>(new Set());

  async function load(): Promise<InstalledApp[]> {
    const list = await invoke<InstalledApp[]>("list_installed_apps");
    setApps(list);
    return list;
  }

  /** 轮询等待指定软件从列表消失（很多卸载器子进程异步执行，进程返回时注册表键还在）。 */
  async function waitGone(ids: string[], timeoutMs = 12000): Promise<InstalledApp[]> {
    const start = Date.now();
    let list = await load();
    while (ids.some((id) => list.some((a) => a.id === id)) && Date.now() - start < timeoutMs) {
      await new Promise((r) => setTimeout(r, 800));
      list = await load();
    }
    return list;
  }

  useEffect(() => {
    load()
      .then(() => setPhase("ready"))
      .catch((e) => {
        setError(String(e));
        setPhase("ready");
      });
  }, []);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return apps;
    return apps.filter(
      (a) =>
        a.name.toLowerCase().includes(q) || a.publisher.toLowerCase().includes(q),
    );
  }, [apps, query]);

  // 按需拉取图标：每个软件只请求一次（按 id 去重）；DisplayIcon 与安装目录均交给后端走兑底链
  useEffect(() => {
    for (const app of filtered) {
      if (requestedIcons.current.has(app.id)) continue;
      if (!app.iconPath && !app.installLocation) continue;
      requestedIcons.current.add(app.id);
      invoke<string | null>("app_icon", {
        iconPath: app.iconPath,
        installLocation: app.installLocation,
        uninstallString: app.uninstallString,
        name: app.name,
      })
        .then((uri) => {
          if (uri) setIcons((prev) => ({ ...prev, [app.id]: uri }));
        })
        .catch(() => {});
    }
  }, [filtered]);

  function toggleSelect(id: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  }

  /** 打开残留弹窗：高置信度项默认勾选，低置信度默认不勾（保守） */
  function openLeftovers(appNames: string[], items: LeftoverItem[]) {
    if (items.length === 0) return;
    setLeftover({ appNames, items });
    setLeftoverChecked(
      new Set(items.filter((it) => it.confidence === "high").map(leftoverKey)),
    );
    setRemoveResults(null);
  }

  /** 卸载单个软件：调原生卸载器 → 轮询确认已消失 → 扫残留 */
  async function uninstallOne(app: InstalledApp) {
    setError("");
    setBusy(`正在等待「${app.name}」的卸载程序完成…`);
    try {
      await invoke<number>("run_app_uninstaller", { id: app.id });
      // 卸载器常在子进程里继续执行，轮询等待它真正从列表消失
      setBusy(`正在确认「${app.name}」的卸载结果…`);
      const after = await waitGone([app.id]);
      // 超时仍在列表：卸载未完成（用户取消/失败），绝不扫描删除其文件
      if (after.some((a) => a.id === app.id)) {
        setBusy("");
        return;
      }
      // 商店应用（UWP）由系统整包移除，无残留需清理
      if (app.id.startsWith("uwp|")) {
        setBusy("");
        return;
      }
      setBusy(`正在扫描「${app.name}」的残留…`);
      const items = await invoke<LeftoverItem[]>("scan_app_leftovers", {
        name: app.name,
        publisher: app.publisher,
        installLocation: app.installLocation,
      });
      setBusy("");
      openLeftovers([app.name], items);
    } catch (e) {
      setError(String(e));
      setBusy("");
    }
  }

  /** 批量卸载（向导模式）：逐个弹原生向导，全部结束后轮询确认并合并扫描残留 */
  async function uninstallBatch() {
    const targets = apps.filter((a) => selected.has(a.id));
    if (targets.length === 0) return;
    setError("");
    try {
      for (let i = 0; i < targets.length; i++) {
        const app = targets[i];
        setBusy(`（${i + 1}/${targets.length}）正在等待「${app.name}」的卸载程序完成…`);
        await invoke<number>("run_app_uninstaller", { id: app.id });
      }
      setBusy("正在确认卸载结果…");
      const after = await waitGone(targets.map((t) => t.id));
      const gone = targets.filter((t) => !after.some((a) => a.id === t.id));
      const merged: LeftoverItem[] = [];
      const seen = new Set<string>();
      for (const app of gone) {
        if (app.id.startsWith("uwp|")) continue; // 商店应用无残留
        const items = await invoke<LeftoverItem[]>("scan_app_leftovers", {
          name: app.name,
          publisher: app.publisher,
          installLocation: app.installLocation,
        });
        for (const it of items) {
          if (seen.add(leftoverKey(it))) merged.push(it);
        }
      }
      setBusy("");
      setSelected(new Set());
      openLeftovers(gone.map((a) => a.name), merged);
    } catch (e) {
      setError(String(e));
      setBusy("");
    }
  }

  function toggleLeftover(key: string) {
    setLeftoverChecked((prev) => {
      const next = new Set(prev);
      next.has(key) ? next.delete(key) : next.add(key);
      return next;
    });
  }

  async function removeLeftovers() {
    if (!leftover) return;
    const items = leftover.items
      .filter((it) => leftoverChecked.has(leftoverKey(it)))
      .map((it) => ({ kind: it.kind, path: it.path }));
    if (items.length === 0) return;
    setRemoving(true);
    try {
      const results = await invoke<RemoveResult[]>("remove_app_leftovers", { items });
      setRemoveResults(results);
    } catch (e) {
      setError(String(e));
    } finally {
      setRemoving(false);
    }
  }

  const dirItems = leftover?.items.filter((it) => it.kind === "dir") ?? [];
  const regItems = leftover?.items.filter((it) => it.kind === "reg") ?? [];
  const selectedCount = selected.size;

  return (
    <div className="flex h-full flex-col">
      {/* 顶部标题 */}
      <header className="px-8 pt-6 pb-2">
        <h1 className="text-xl font-semibold">软件卸载</h1>
        <p className="mt-0.5 text-sm text-[var(--color-text-secondary)]">
          彻底卸载软件并清理残留文件与注册表项，支持批量。残留删除前会先备份、文件可从回收站找回
        </p>
      </header>

      {/* 控制条 */}
      <div className="mx-8 mt-2 flex items-center gap-4 rounded-2xl bg-[var(--color-surface)] p-4 shadow-[var(--shadow-card)]">
        <div className="relative flex-1">
          <Search
            size={18}
            className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[var(--color-text-secondary)]"
          />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="搜索软件名称或发行商…"
            className="w-full rounded-xl border border-[var(--color-line)] bg-[var(--color-bg)] py-2.5 pl-10 pr-3 text-sm outline-none focus:border-[var(--color-primary)]"
          />
        </div>
        <span className="shrink-0 text-sm text-[var(--color-text-secondary)]">
          共 <span className="font-semibold text-[var(--color-text-main)]">{apps.length}</span> 个
        </span>
        <button
          onClick={() => {
            requestedIcons.current.clear();
            load().catch((e) => setError(String(e)));
          }}
          disabled={!!busy}
          title="刷新列表"
          className="flex shrink-0 items-center gap-1.5 rounded-xl border border-[var(--color-line)] px-3 py-2.5 text-sm transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary-dark)] disabled:opacity-40"
        >
          <RotateCcw size={16} />
          刷新
        </button>
        {selectedCount > 0 && (
          <button
            onClick={() => setSelected(new Set())}
            disabled={!!busy}
            title="取消全部勾选"
            className="flex shrink-0 items-center gap-1.5 rounded-xl border border-[var(--color-line)] px-3 py-2.5 text-sm transition-colors hover:border-[var(--color-caution)] hover:text-[var(--color-caution)] disabled:opacity-40"
          >
            <XCircle size={16} />
            清空 ({selectedCount})
          </button>
        )}
        <button
          onClick={uninstallBatch}
          disabled={selectedCount < 2 || !!busy}
          className="flex shrink-0 items-center gap-2 rounded-xl bg-[var(--color-primary)] px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)] disabled:opacity-40"
        >
          <Trash2 size={16} />
          批量卸载{selectedCount >= 2 ? ` (${selectedCount})` : ""}
        </button>
      </div>

      {/* 主体：软件列表 */}
      <div className="flex-1 overflow-y-auto px-8 py-5">
        {error && (
          <div className="mb-4 rounded-xl bg-[var(--color-surface)] px-4 py-3 text-sm text-[var(--color-caution)] shadow-[var(--shadow-card)]">
            出了点问题：{error}
          </div>
        )}

        {phase === "loading" ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-[var(--color-text-secondary)]">
            <Loader2 size={32} className="animate-spin text-[var(--color-primary)]" />
            正在读取已安装的软件…
          </div>
        ) : filtered.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-[var(--color-text-secondary)]">
            <Package size={36} className="opacity-40" />
            {query ? "没有匹配的软件" : "没有找到可卸载的软件"}
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            {filtered.map((app, i) => {
              const active = selected.has(app.id);
              return (
                <div
                  key={`${app.id}#${i}`}
                  className={[
                    "flex items-center gap-3 rounded-xl border bg-[var(--color-surface)] px-4 py-3 transition-colors",
                    active
                      ? "border-[var(--color-primary)]"
                      : "border-[var(--color-line)] hover:border-[var(--color-primary-soft)]",
                  ].join(" ")}
                >
                  <input
                    type="checkbox"
                    checked={active}
                    onChange={() => toggleSelect(app.id)}
                    className="h-4 w-4 shrink-0 accent-[var(--color-primary)]"
                  />
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-[var(--color-primary-soft)] text-[var(--color-primary-dark)]">
                    {icons[app.id] ? (
                      <img
                        src={icons[app.id]}
                        alt=""
                        className="h-7 w-7 object-contain"
                        draggable={false}
                      />
                    ) : (
                      <Package size={18} />
                    )}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium">
                      {app.name}
                      {app.version && (
                        <span className="ml-2 text-xs font-normal text-[var(--color-text-secondary)]">
                          {app.version}
                        </span>
                      )}
                    </div>
                    <div className="truncate text-xs text-[var(--color-text-secondary)]">
                      {app.publisher || "未知发行商"}
                    </div>
                  </div>
                  {app.size > 0 && (
                    <span className="shrink-0 text-xs text-[var(--color-text-secondary)]">
                      {formatBytes(app.size)}
                    </span>
                  )}
                  <button
                    onClick={() => uninstallOne(app)}
                    disabled={!!busy}
                    className="flex shrink-0 items-center gap-1.5 rounded-lg border border-[var(--color-line)] px-3 py-1.5 text-xs font-medium transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary-dark)] disabled:opacity-40"
                  >
                    卸载
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* 卸载执行中遮罩 */}
      <AnimatePresence>
        {busy && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="absolute inset-0 z-20 flex items-center justify-center bg-black/20"
          >
            <div className="flex items-center gap-3 rounded-2xl bg-[var(--color-surface)] px-6 py-4 shadow-[var(--shadow-card)]">
              <Loader2 size={20} className="animate-spin text-[var(--color-primary)]" />
              <span className="text-sm">{busy}</span>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* 残留清单弹窗 */}
      <AnimatePresence>
        {leftover && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="absolute inset-0 z-30 flex items-center justify-center bg-black/30 p-8"
            onClick={() => !removing && setLeftover(null)}
          >
            <motion.div
              initial={{ scale: 0.96, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.96, opacity: 0 }}
              onClick={(e) => e.stopPropagation()}
              className="flex max-h-full w-full max-w-2xl flex-col overflow-hidden rounded-2xl bg-[var(--color-surface)] shadow-[var(--shadow-card)]"
            >
              {/* 弹窗头 */}
              <div className="border-b border-[var(--color-line)] px-6 py-4">
                <div className="flex items-center gap-2 text-base font-semibold">
                  <ShieldCheck size={18} className="text-[var(--color-primary)]" />
                  卸载完成，发现以下残留
                </div>
                <p className="mt-1 text-xs text-[var(--color-text-secondary)]">
                  已卸载：{leftover.appNames.join("、")}。普通文件删入回收站可恢复；系统目录与注册表需管理员授权（会弹一次 UAC），删注册表前自动导出 .reg 备份
                </p>
              </div>

              {/* 残留列表 */}
              <div className="flex-1 overflow-y-auto px-6 py-4">
                {removeResults ? (
                  <ResultList results={removeResults} />
                ) : leftover.items.length === 0 ? (
                  <div className="py-8 text-center text-sm text-[var(--color-text-secondary)]">
                    很干净，没有发现明显残留 🎉
                  </div>
                ) : (
                  <div className="flex flex-col gap-4">
                    {dirItems.length > 0 && (
                      <LeftoverGroup
                        title="残留文件夹"
                        icon={<FolderOpen size={15} />}
                        items={dirItems}
                        checked={leftoverChecked}
                        onToggle={toggleLeftover}
                      />
                    )}
                    {regItems.length > 0 && (
                      <LeftoverGroup
                        title="残留注册表项"
                        icon={<KeyRound size={15} />}
                        items={regItems}
                        checked={leftoverChecked}
                        onToggle={toggleLeftover}
                      />
                    )}
                  </div>
                )}
              </div>

              {/* 弹窗底部操作 */}
              <div className="flex items-center justify-end gap-3 border-t border-[var(--color-line)] px-6 py-4">
                <button
                  onClick={() => setLeftover(null)}
                  disabled={removing}
                  className="rounded-xl px-4 py-2 text-sm text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg)] disabled:opacity-40"
                >
                  {removeResults ? "关闭" : "跳过残留"}
                </button>
                {!removeResults && leftover.items.length > 0 && (
                  <button
                    onClick={removeLeftovers}
                    disabled={removing || leftoverChecked.size === 0}
                    className="flex items-center gap-2 rounded-xl bg-[var(--color-primary)] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)] disabled:opacity-40"
                  >
                    {removing ? <Loader2 size={16} className="animate-spin" /> : <Trash2 size={16} />}
                    删除选中残留 ({leftoverChecked.size})
                  </button>
                )}
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

/** 一组残留项（文件夹 / 注册表） */
function LeftoverGroup({
  title,
  icon,
  items,
  checked,
  onToggle,
}: {
  title: string;
  icon: React.ReactNode;
  items: LeftoverItem[];
  checked: Set<string>;
  onToggle: (key: string) => void;
}) {
  return (
    <div>
      <div className="mb-2 flex items-center gap-1.5 text-xs font-medium text-[var(--color-text-secondary)]">
        {icon}
        {title}（{items.length}）
      </div>
      <div className="flex flex-col gap-1.5">
        {items.map((it) => {
          const key = leftoverKey(it);
          const low = it.confidence === "low";
          return (
            <label
              key={key}
              className="flex cursor-pointer items-center gap-2.5 rounded-lg border border-[var(--color-line)] bg-[var(--color-bg)] px-3 py-2"
            >
              <input
                type="checkbox"
                checked={checked.has(key)}
                onChange={() => onToggle(key)}
                className="h-4 w-4 shrink-0 accent-[var(--color-primary)]"
              />
              <div className="min-w-0 flex-1">
                <div className="truncate text-xs text-[var(--color-text-main)]" title={it.path}>
                  {it.path}
                </div>
              </div>
              {low ? (
                <span className="flex shrink-0 items-center gap-1 text-[11px] text-[var(--color-caution)]">
                  <AlertTriangle size={12} />
                  谨慎
                </span>
              ) : (
                it.kind === "dir" &&
                it.size > 0 && (
                  <span className="shrink-0 text-[11px] text-[var(--color-text-secondary)]">
                    {formatBytes(it.size)}
                  </span>
                )
              )}
            </label>
          );
        })}
      </div>
    </div>
  );
}

/** 删除结果列表 */
function ResultList({ results }: { results: RemoveResult[] }) {
  const ok = results.filter((r) => r.ok).length;
  const fail = results.length - ok;
  return (
    <div className="flex flex-col gap-3">
      <div className="text-sm">
        已删除 <span className="font-semibold text-[var(--color-safe)]">{ok}</span> 项
        {fail > 0 && (
          <>
            ，<span className="font-semibold text-[var(--color-caution)]">{fail}</span> 项未成功
          </>
        )}
      </div>
      <div className="flex flex-col gap-1.5">
        {results.map((r) => (
          <div
            key={r.path}
            className="flex items-center gap-2 rounded-lg border border-[var(--color-line)] bg-[var(--color-bg)] px-3 py-2"
          >
            {r.ok ? (
              <CheckCircle2 size={14} className="shrink-0 text-[var(--color-safe)]" />
            ) : (
              <XCircle size={14} className="shrink-0 text-[var(--color-caution)]" />
            )}
            <span className="min-w-0 flex-1 truncate text-xs" title={r.path}>
              {r.path}
            </span>
            {!r.ok && r.error && (
              <span className="shrink-0 text-[11px] text-[var(--color-caution)]">{r.error}</span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
