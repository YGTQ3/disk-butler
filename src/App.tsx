import { useState } from "react";
import { HardDrive, Sparkles, Rocket, Cpu, ShieldCheck, HeartHandshake } from "lucide-react";
import DiskInsight from "./pages/DiskInsight";
import Cleanup from "./pages/Cleanup";
import Startup from "./pages/Startup";
import MemoryCheck from "./pages/MemoryCheck";
import ContributeModal from "./components/ContributeModal";

type PageId = "insight" | "clean" | "startup" | "memory";

interface NavItem {
  id: PageId;
  label: string;
  icon: React.ReactNode;
  ready: boolean;
}

const NAV_ITEMS: NavItem[] = [
  { id: "insight", label: "磁盘透视", icon: <HardDrive size={20} />, ready: true },
  { id: "clean", label: "一键清理", icon: <Sparkles size={20} />, ready: true },
  { id: "startup", label: "启动管理", icon: <Rocket size={20} />, ready: true },
  { id: "memory", label: "内存体检", icon: <Cpu size={20} />, ready: true },
];

function App() {
  const [page, setPage] = useState<PageId>("insight");
  // 访问过的页面保持挂载（只隐藏不销毁），切页不丢扫描结果
  const [visited, setVisited] = useState<Set<PageId>>(new Set(["insight"]));
  const [showContribute, setShowContribute] = useState(false);

  function go(id: PageId) {
    setVisited((prev) => {
      if (prev.has(id)) return prev;
      const next = new Set(prev);
      next.add(id);
      return next;
    });
    setPage(id);
  }

  return (
    <div className="flex h-full w-full overflow-hidden">
      {/* 左侧窄导航 */}
      <aside className="flex w-56 shrink-0 flex-col border-r border-[var(--color-line)] bg-[var(--color-surface)]">
        {/* 品牌区 */}
        <div className="flex items-center gap-2.5 px-5 py-5">
          <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-[var(--color-primary)] text-white">
            <ShieldCheck size={20} />
          </div>
          <div>
            <div className="text-base font-semibold leading-tight">C盘管家</div>
            <div className="text-xs text-[var(--color-text-secondary)]">磁盘 · 内存维护助手</div>
          </div>
        </div>

        {/* 导航项 */}
        <nav className="flex flex-col gap-1 px-3 py-2">
          {NAV_ITEMS.map((item) => {
            const active = page === item.id;
            return (
              <button
                key={item.id}
                disabled={!item.ready}
                onClick={() => item.ready && go(item.id)}
                className={[
                  "group flex items-center gap-3 rounded-xl px-3 py-2.5 text-left text-sm transition-colors",
                  active
                    ? "bg-[var(--color-primary-soft)] text-[var(--color-primary-dark)] font-medium"
                    : item.ready
                      ? "text-[var(--color-text-main)] hover:bg-[var(--color-bg)]"
                      : "cursor-not-allowed text-[var(--color-text-secondary)] opacity-50",
                ].join(" ")}
              >
                {item.icon}
                <span className="flex-1">{item.label}</span>
                {!item.ready && (
                  <span className="rounded-full bg-[var(--color-bg)] px-2 py-0.5 text-[10px] text-[var(--color-text-secondary)]">
                    即将推出
                  </span>
                )}
              </button>
            );
          })}
        </nav>

        <div className="mt-auto px-3 pb-2">
          {/* 社区贡献入口：本地生成报告，发不发由用户决定 */}
          <button
            onClick={() => setShowContribute(true)}
            className="flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left text-sm text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg)] hover:text-[var(--color-text-main)]"
          >
            <HeartHandshake size={20} />
            <span className="flex-1">帮它认识更多软件</span>
          </button>
        </div>

        <div className="px-5 pb-4 text-[11px] leading-relaxed text-[var(--color-text-secondary)]">
          v{__APP_VERSION__.split(".").slice(0, 2).join(".")} · 透视 / 清理 / 启动 / 内存
          <br />
          所有操作都会先告诉你“这是什么”
        </div>
      </aside>

      {/* 主内容区：访问过的页面常驻内存，切换仅切可见性 */}
      <main className="relative flex-1 overflow-hidden">
        <div className={page === "insight" ? "h-full" : "hidden"}>
          <DiskInsight active={page === "insight"} />
        </div>
        {visited.has("clean") && (
          <div className={page === "clean" ? "h-full" : "hidden"}>
            <Cleanup />
          </div>
        )}
        {visited.has("startup") && (
          <div className={page === "startup" ? "h-full" : "hidden"}>
            <Startup />
          </div>
        )}
        {visited.has("memory") && (
          <div className={page === "memory" ? "h-full" : "hidden"}>
            <MemoryCheck />
          </div>
        )}
      </main>

      {showContribute && <ContributeModal onClose={() => setShowContribute(false)} />}
    </div>
  );
}

export default App;
