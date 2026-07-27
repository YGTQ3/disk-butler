import { useEffect, useRef, useState, useLayoutEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { motion } from "framer-motion";
import { HardDrive, Search, ChevronRight, Loader2, RotateCcw } from "lucide-react";
import { DriveInfo, TreeNode, ScanProgress, ScanCache, formatBytes, formatCount, formatAgo } from "../types";
import CapacityBar from "../components/CapacityBar";
import TreeMap from "../components/TreeMap";
import DetailPanel from "../components/DetailPanel";

type Phase = "idle" | "scanning" | "done";

interface Props {
  /** 页面当前是否可见（保持挂载方案下，重新可见时需重测 TreeMap 尺寸） */
  active?: boolean;
}

export default function DiskInsight({ active = true }: Props) {
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [selected, setSelected] = useState<string>("");
  const [phase, setPhase] = useState<Phase>("idle");
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [root, setRoot] = useState<TreeNode | null>(null);
  /** 当前扫描结果属于哪个盘（挂载点）——标题/面包屑跟结果走，不跟下拉框走 */
  const [scannedDrive, setScannedDrive] = useState<string>("");
  /** 结果的扫描时刻（Unix 秒），用于“扫描于 X 前”标注 */
  const [scannedAt, setScannedAt] = useState<number | null>(null);
  /** 当前结果是否来自本地缓存（而非本次会话的新扫描） */
  const [fromCache, setFromCache] = useState(false);
  const [stack, setStack] = useState<TreeNode[]>([]); // 下钻路径栈
  const [selectedNode, setSelectedNode] = useState<TreeNode | null>(null);
  const [error, setError] = useState<string>("");

  const mapAreaRef = useRef<HTMLDivElement>(null);
  const [mapSize, setMapSize] = useState({ w: 640, h: 420 });

  /** 尝试加载某盘的本地缓存；命中则直接展示（不自动重扫，更新由用户手动触发） */
  async function tryLoadCache(mount: string): Promise<boolean> {
    try {
      const cache = await invoke<ScanCache | null>("load_scan_cache", { root: mount });
      if (!cache) return false;
      setRoot(cache.tree);
      setScannedDrive(cache.root);
      setScannedAt(cache.scannedAt);
      setFromCache(true);
      setStack([]);
      setSelectedNode(null);
      setPhase("done");
      return true;
    } catch {
      return false;
    }
  }

  // 加载盘符，并优先恢复默认盘的上次扫描结果
  useEffect(() => {
    invoke<DriveInfo[]>("get_drives")
      .then((ds) => {
        setDrives(ds);
        const c = ds.find((d) => d.letter.toUpperCase() === "C") ?? ds[0];
        if (c) {
          setSelected(c.mountPoint);
          tryLoadCache(c.mountPoint);
        }
      })
      .catch((e) => setError(String(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 监听扫描进度
  useEffect(() => {
    const un = listen<ScanProgress>("scan-progress", (e) => {
      setProgress(e.payload);
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  // 测量 TreeMap 可用区域（页面重新可见时也重测，避免隐藏期间窗口缩放导致尺寸归零）
  useLayoutEffect(() => {
    if (phase !== "done" || !active) return;
    const measure = () => {
      const el = mapAreaRef.current;
      if (el && el.clientWidth > 0) setMapSize({ w: el.clientWidth, h: el.clientHeight });
    };
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [phase, active]);

  const current = stack.length > 0 ? stack[stack.length - 1] : root;
  const currentDrive = drives.find((d) => d.mountPoint === selected);
  const scannedDriveInfo = drives.find((d) => d.mountPoint === scannedDrive);
  /** 选中盘 ≠ 已扫描盘：结果与选择不一致，需提示用户扫描新盘 */
  const driveMismatch = phase === "done" && !!scannedDrive && selected !== scannedDrive;

  async function startScan() {
    if (!selected) return;
    const target = selected; // 锁定本次扫描目标，避免扫描中切换下拉框干扰
    setPhase("scanning");
    setProgress(null);
    setError("");
    setRoot(null);
    setStack([]);
    setSelectedNode(null);
    try {
      const tree = await invoke<TreeNode>("scan_drive", { root: target });
      setRoot(tree);
      setScannedDrive(target);
      setScannedAt(Math.floor(Date.now() / 1000));
      setFromCache(false);
      setPhase("done");
    } catch (e) {
      setError(String(e));
      setPhase("idle");
    }
  }

  /** 切换盘符：优先找该盘的缓存，找不到则保持现状（由不匹配提示条引导扫描） */
  function onSelectDrive(mount: string) {
    setSelected(mount);
    if (phase !== "scanning" && mount !== scannedDrive) {
      tryLoadCache(mount);
    }
  }

  function drill(node: TreeNode) {
    // 若下钻目标层级已被剪枝（无 children 但 hasChildren），按需二次扫描
    if ((!node.children || node.children.length === 0) && node.hasChildren) {
      invoke<TreeNode>("scan_dir", { path: node.path })
        .then((sub) => setStack((s) => [...s, sub]))
        .catch((e) => setError(String(e)));
    } else {
      setStack((s) => [...s, node]);
    }
  }

  function jumpTo(index: number) {
    // index = -1 表示回到根
    if (index < 0) setStack([]);
    else setStack((s) => s.slice(0, index + 1));
    setSelectedNode(null);
  }

  // 面包屑：根 + 栈
  const crumbs = [root, ...stack].filter(Boolean) as TreeNode[];

  return (
    <div className="flex h-full flex-col">
      {/* 顶部标题 */}
      <header className="px-8 pt-6 pb-2">
        <h1 className="text-xl font-semibold">磁盘透视</h1>
        <p className="mt-0.5 text-sm text-[var(--color-text-secondary)]">
          看清每一寸空间被谁占用，点击方块可以层层深入
        </p>
      </header>

      {/* 控制条 */}
      <div className="mx-8 mt-2 rounded-2xl bg-[var(--color-surface)] p-5 shadow-[var(--shadow-card)]">
        <div className="flex items-center gap-4">
          {/* 盘符选择 */}
          <div className="relative">
            <select
              value={selected}
              onChange={(e) => onSelectDrive(e.target.value)}
              disabled={phase === "scanning"}
              className="appearance-none rounded-xl border border-[var(--color-line)] bg-[var(--color-bg)] py-2.5 pl-10 pr-8 text-sm font-medium outline-none focus:border-[var(--color-primary)] disabled:opacity-50"
            >
              {drives.map((d) => (
                <option key={d.mountPoint} value={d.mountPoint}>
                  {d.letter} 盘 ({formatBytes(d.total)})
                </option>
              ))}
            </select>
            <HardDrive
              size={18}
              className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[var(--color-text-secondary)]"
            />
          </div>

          {/* 水位条：盘与结果不匹配时不用旧结果给新盘着色 */}
          <div className="flex-1">
            {currentDrive && (
              <CapacityBar
                root={driveMismatch ? null : root}
                total={currentDrive.total}
                free={currentDrive.free}
              />
            )}
          </div>

          {/* 扫描按钮 */}
          <button
            onClick={startScan}
            disabled={phase === "scanning" || !selected}
            className="flex items-center gap-2 rounded-xl bg-[var(--color-primary)] px-5 py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)] disabled:opacity-50"
          >
            {phase === "scanning" ? (
              <Loader2 size={18} className="animate-spin" />
            ) : phase === "done" && !driveMismatch ? (
              <RotateCcw size={18} />
            ) : (
              <Search size={18} />
            )}
            {phase === "scanning"
              ? "扫描中…"
              : driveMismatch
                ? `扫描 ${currentDrive?.letter ?? ""} 盘`
                : phase === "done"
                  ? "重新扫描"
                  : "开始扫描"}
          </button>
        </div>
      </div>

      {/* 主体区 */}
      <div className="flex flex-1 gap-5 overflow-hidden p-8 pt-5">
        {error && (
          <div className="flex flex-1 items-center justify-center">
            <div className="rounded-xl bg-[var(--color-surface)] px-6 py-4 text-sm text-[var(--color-caution)] shadow-[var(--shadow-card)]">
              出了点问题：{error}
            </div>
          </div>
        )}

        {/* 空状态 */}
        {!error && phase === "idle" && (
          <div className="flex flex-1 flex-col items-center justify-center gap-4">
            <motion.div
              initial={{ scale: 0.9, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              className="flex h-20 w-20 items-center justify-center rounded-3xl bg-[var(--color-primary-soft)]"
            >
              <HardDrive size={36} className="text-[var(--color-primary)]" />
            </motion.div>
            <div className="text-center">
              <div className="text-base font-medium">选择磁盘，点击"开始扫描"</div>
              <div className="mt-1 text-sm text-[var(--color-text-secondary)]">
                扫描完全在你的电脑本地进行，不上传任何数据
              </div>
            </div>
          </div>
        )}

        {/* 扫描中 */}
        {!error && phase === "scanning" && (
          <div className="flex flex-1 flex-col items-center justify-center gap-5">
            <Loader2 size={40} className="animate-spin text-[var(--color-primary)]" />
            <div className="text-center">
              <div className="text-base font-medium">
                正在扫描 {progress ? `已发现 ${formatCount(progress.filesScanned)} 个文件` : "…"}
              </div>
              <div className="mt-1 max-w-lg truncate text-xs text-[var(--color-text-secondary)]">
                {progress?.currentPath || "正在准备…"}
              </div>
              {progress && (
                <div className="mt-2 text-sm text-[var(--color-primary-dark)]">
                  已统计 {formatBytes(progress.bytesScanned)}
                </div>
              )}
            </div>
          </div>
        )}

        {/* 完成：TreeMap + 详情卡 */}
        {!error && phase === "done" && current && (
          <>
            <div className="flex flex-1 flex-col overflow-hidden rounded-2xl bg-[var(--color-surface)] p-5 shadow-[var(--shadow-card)]">
              {/* 选中盘与结果不一致时的提示条 */}
              {driveMismatch && (
                <div className="mb-3 rounded-xl bg-[#FEF3C7] px-4 py-2.5 text-xs leading-relaxed text-[#92400E]">
                  下方显示的是 <b>{scannedDriveInfo?.letter ?? "?"} 盘</b> 的扫描结果；想查看{" "}
                  <b>{currentDrive?.letter ?? ""} 盘</b>，请点右上方「扫描 {currentDrive?.letter ?? ""} 盘」。
                </div>
              )}
              {/* 面包屑（盘符标签跟扫描结果走，不跟下拉框走）+ 扫描时间标注 */}
              <div className="mb-4 flex flex-wrap items-center gap-1 text-sm">
                {crumbs.map((c, i) => (
                  <div key={c.path} className="flex items-center gap-1">
                    {i > 0 && <ChevronRight size={14} className="text-[var(--color-text-secondary)]" />}
                    <button
                      onClick={() => jumpTo(i - 1)}
                      className={[
                        "rounded-lg px-2 py-1 transition-colors hover:bg-[var(--color-bg)]",
                        i === crumbs.length - 1
                          ? "font-medium text-[var(--color-text-main)]"
                          : "text-[var(--color-text-secondary)]",
                      ].join(" ")}
                    >
                      {i === 0 ? `${scannedDriveInfo?.letter ?? ""} 盘` : c.name}
                    </button>
                  </div>
                ))}
                {scannedAt !== null && (
                  <span
                    className="ml-auto rounded-full bg-[var(--color-bg)] px-2.5 py-1 text-[11px] text-[var(--color-text-secondary)]"
                    title={fromCache ? "这是上次扫描保存的结果，点“重新扫描”获取最新数据" : "本次扫描的结果"}
                  >
                    扫描于 {formatAgo(scannedAt)}
                    {fromCache && " · 上次结果"}
                  </span>
                )}
              </div>

              {/* TreeMap 画布 */}
              <div ref={mapAreaRef} className="relative flex-1 overflow-hidden">
                <TreeMap
                  key={current.path}
                  node={current}
                  width={mapSize.w}
                  height={mapSize.h}
                  onDrill={drill}
                  onSelect={setSelectedNode}
                />
              </div>
            </div>

            {/* 详情卡 */}
            <div className="w-80 shrink-0 overflow-y-auto rounded-2xl bg-[var(--color-surface)] shadow-[var(--shadow-card)]">
              <DetailPanel node={selectedNode} rootSize={current.size} />
            </div>
          </>
        )}
      </div>
    </div>
  );
}
