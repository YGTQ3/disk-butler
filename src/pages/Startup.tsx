import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, RotateCcw, Rocket, ShieldCheck, Scale } from "lucide-react";
import { StartupItem, formatBytes } from "../types";

type Phase = "loading" | "ready";

const ADVICE_META = {
  disable: {
    title: "建议禁用",
    subtitle: "禁用后不影响手动使用，开机更快更省内存",
    icon: <Rocket size={15} />,
    badge: { text: "建议禁用", bg: "#FEE2E2", fg: "#991B1B" },
  },
  neutral: {
    title: "看你习惯",
    subtitle: "有实际用途，按自己的使用频率决定",
    icon: <Scale size={15} />,
    badge: { text: "看习惯", bg: "#FEF3C7", fg: "#92400E" },
  },
  keep: {
    title: "建议保留",
    subtitle: "系统功能、安全软件或轻量常用工具",
    icon: <ShieldCheck size={15} />,
    badge: { text: "建议保留", bg: "#D1FAE5", fg: "#065F46" },
  },
} as const;

export default function Startup() {
  const [phase, setPhase] = useState<Phase>("loading");
  const [items, setItems] = useState<StartupItem[]>([]);
  const [error, setError] = useState("");
  /** 每个条目最近一次切换的报错（如需要管理员） */
  const [itemErrors, setItemErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);

  async function load() {
    setPhase("loading");
    setError("");
    setItemErrors({});
    try {
      setItems(await invoke<StartupItem[]>("list_startup_items"));
    } catch (e) {
      setError(String(e));
    }
    setPhase("ready");
  }

  useEffect(() => {
    load();
  }, []);

  async function toggle(item: StartupItem) {
    setBusy(item.id);
    setItemErrors((prev) => ({ ...prev, [item.id]: "" }));
    try {
      await invoke("set_startup_enabled", { id: item.id, enabled: !item.enabled });
      setItems((prev) =>
        prev.map((i) => (i.id === item.id ? { ...i, enabled: !item.enabled } : i)),
      );
    } catch (e) {
      setItemErrors((prev) => ({ ...prev, [item.id]: String(e) }));
    }
    setBusy(null);
  }

  /** 分组批量启用/禁用（逐项执行，失败项单独显示错误） */
  async function setGroupEnabled(advice: string, enabled: boolean) {
    const targets = items.filter((i) => i.advice === advice && i.enabled !== enabled);
    for (const item of targets) {
      setBusy(item.id);
      try {
        await invoke("set_startup_enabled", { id: item.id, enabled });
        setItems((prev) => prev.map((i) => (i.id === item.id ? { ...i, enabled } : i)));
        setItemErrors((prev) => ({ ...prev, [item.id]: "" }));
      } catch (e) {
        setItemErrors((prev) => ({ ...prev, [item.id]: String(e) }));
      }
    }
    setBusy(null);
  }

  const enabledCount = items.filter((i) => i.enabled).length;
  const suggestible = items.filter((i) => i.advice === "disable" && i.enabled).length;

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-start justify-between px-8 pt-6 pb-2">
        <div>
          <h1 className="text-xl font-semibold">启动管理</h1>
          <p className="mt-0.5 text-sm text-[var(--color-text-secondary)]">
            开机自启越少，开机越快、内存越省。禁用是可逆的，随时能改回来
          </p>
        </div>
        {phase === "ready" && (
          <button
            onClick={load}
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
            正在读取启动项与运行状态…
          </div>
        </div>
      )}

      {phase === "ready" && (
        <div className="flex-1 overflow-y-auto px-8 pb-6 pt-2">
          {error && (
            <div className="mb-4 rounded-xl bg-[var(--color-surface)] px-4 py-3 text-sm text-[var(--color-caution)] shadow-[var(--shadow-card)]">
              {error}
            </div>
          )}

          {/* 概况条 */}
          <div className="mb-5 flex items-center gap-6 rounded-2xl bg-[var(--color-surface)] px-5 py-3.5 shadow-[var(--shadow-card)] text-sm">
            <span>
              共 <b>{items.length}</b> 个启动项，<b>{enabledCount}</b> 个已启用
            </span>
            {suggestible > 0 && (
              <span className="text-[var(--color-caution)]">
                其中 <b>{suggestible}</b> 个建议禁用 ↓
              </span>
            )}
          </div>

          {(["disable", "neutral", "keep"] as const).map((advice) => {
            const group = items.filter((i) => i.advice === advice);
            if (group.length === 0) return null;
            const meta = ADVICE_META[advice];
            return (
              <div key={advice} className="mb-6">
                <div className="mb-2 flex items-center gap-2">
                  <span className="flex items-center gap-1.5 text-sm font-semibold">
                    {meta.icon}
                    {meta.title}
                  </span>
                  <span className="text-xs text-[var(--color-text-secondary)]">
                    {meta.subtitle}
                  </span>
                  {/* 分组批量开关 */}
                  <div className="ml-auto flex gap-1.5">
                    <button
                      onClick={() => setGroupEnabled(advice, false)}
                      className="rounded-lg border border-[var(--color-line)] px-2.5 py-1 text-[11px] text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-caution)] hover:text-[var(--color-caution)]"
                    >
                      全部禁用
                    </button>
                    <button
                      onClick={() => setGroupEnabled(advice, true)}
                      className="rounded-lg border border-[var(--color-line)] px-2.5 py-1 text-[11px] text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary-dark)]"
                    >
                      全部启用
                    </button>
                  </div>
                </div>
                <div className="space-y-2.5">
                  {group.map((item) => (
                    <div
                      key={item.id}
                      className="rounded-2xl bg-[var(--color-surface)] p-4 shadow-[var(--shadow-card)]"
                    >
                      <div className="flex items-center gap-3">
                        {/* 开关 */}
                        <button
                          onClick={() => toggle(item)}
                          disabled={busy === item.id}
                          title={item.enabled ? "点击禁用" : "点击启用"}
                          className={[
                            "relative h-5.5 w-10 shrink-0 rounded-full transition-colors",
                            item.enabled
                              ? "bg-[var(--color-primary)]"
                              : "bg-[var(--color-line)]",
                          ].join(" ")}
                          style={{ height: 22 }}
                        >
                          <span
                            className="absolute top-0.5 h-[18px] w-[18px] rounded-full bg-white shadow transition-all"
                            style={{ left: item.enabled ? 20 : 2 }}
                          />
                        </button>

                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2">
                            {item.icon && (
                              <img src={item.icon} alt="" className="h-5 w-5 shrink-0 rounded-sm" />
                            )}
                            <span className="truncate text-sm font-semibold">{item.name}</span>
                            <span
                              className="rounded-full px-2 py-0.5 text-[10px] font-medium"
                              style={{ background: meta.badge.bg, color: meta.badge.fg }}
                            >
                              {meta.badge.text}
                            </span>
                            <span className="rounded-full bg-[var(--color-bg)] px-2 py-0.5 text-[10px] text-[var(--color-text-secondary)]">
                              {item.location}
                            </span>
                            {item.needsAdmin && (
                              <span className="rounded-full bg-[var(--color-bg)] px-2 py-0.5 text-[10px] text-[var(--color-text-secondary)]">
                                改动需管理员
                              </span>
                            )}
                            {item.memMb > 0 && (
                              <span className="ml-auto shrink-0 text-xs font-bold text-[var(--color-primary-dark)]">
                                运行中 {formatBytes(item.memMb * 1024 * 1024)}
                              </span>
                            )}
                          </div>
                          <div className="mt-1 text-xs leading-relaxed">{item.reason}</div>
                          <div
                            className="mt-1 truncate text-[11px] text-[var(--color-text-secondary)]"
                            title={item.command}
                          >
                            {item.command}
                          </div>
                          {itemErrors[item.id] && (
                            <div className="mt-1.5 text-xs text-[var(--color-caution)]">
                              {itemErrors[item.id]}
                            </div>
                          )}
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
