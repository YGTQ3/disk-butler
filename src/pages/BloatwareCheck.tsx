import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "framer-motion";
import {
  Loader2,
  RotateCcw,
  ShieldCheck,
  AlertTriangle,
  FolderOpen,
  Trash2,
  CheckCircle2,
  BellOff,
  Bell,
  ChevronDown,
  ChevronRight,
} from "lucide-react";
import { formatBytes, openInExplorer } from "../types";

interface BloatwareEntry {
  id: string;
  key: string;
  name: string;
  publisher: string;
  version: string;
  installLocation: string;
  sizeMb: number | null;
  tags: string[];
  behaviors: string[];
  suggestion: string;
  trusted: boolean;
  dismissed: boolean;
  uninstallable: boolean;
  autostartCount: number;
  residentMemMb: number;
}

interface BloatwareScan {
  entries: BloatwareEntry[];
  shownCount: number;
  browserNotes: string[];
}

interface ResidueDetail {
  path: string;
  size: number;
}

type Phase = "loading" | "ready";

const TAG_STYLE: Record<string, { bg: string; fg: string }> = {
  开机自启: { bg: "#EEF2FF", fg: "#3730A3" },
  后台常驻: { bg: "#ECFEFF", fg: "#155E75" },
  占用较大: { bg: "#F1F5F9", fg: "#475569" },
};
const DEFAULT_TAG = { bg: "#F1F5F9", fg: "#475569" };

/** MB 数 -> 人话大小；null 显示占位 */
function sizeText(mb: number | null): string {
  if (mb == null || mb <= 0) return "—";
  return formatBytes(mb * 1024 * 1024);
}

export default function BloatwareCheck() {
  const [phase, setPhase] = useState<Phase>("loading");
  const [scan, setScan] = useState<BloatwareScan | null>(null);
  const [error, setError] = useState("");

  const [confirmTarget, setConfirmTarget] = useState<BloatwareEntry | null>(null);
  const [busyId, setBusyId] = useState("");
  const [msg, setMsg] = useState("");

  const [residue, setResidue] = useState<{ name: string; detail: ResidueDetail } | null>(null);
  const [cleaningResidue, setCleaningResidue] = useState(false);

  // 折叠区展开状态
  const [showTrusted, setShowTrusted] = useState(false);
  const [showDismissed, setShowDismissed] = useState(false);

  async function load() {
    setPhase("loading");
    setError("");
    try {
      setScan(await invoke<BloatwareScan>("scan_bloatware"));
    } catch (e) {
      setError(String(e));
    }
    setPhase("ready");
  }

  useEffect(() => {
    load();
  }, []);

  /** 记录/取消"不再提醒" */
  async function setIgnored(e: BloatwareEntry, ignored: boolean) {
    try {
      await invoke("bloatware_set_ignored", { key: e.key, ignored });
      await load();
    } catch (err) {
      setMsg(String(err));
    }
  }

  async function doUninstall(entry: BloatwareEntry) {
    setConfirmTarget(null);
    setBusyId(entry.id);
    setMsg("");
    try {
      await invoke("uninstall_software", { id: entry.id });
      setMsg(`已卸载「${entry.name}」。`);
      if (entry.installLocation) {
        try {
          const detail = await invoke<ResidueDetail | null>("scan_residue", {
            installLocation: entry.installLocation,
          });
          if (detail) setResidue({ name: entry.name, detail });
        } catch {
          /* 残留探测失败不打扰 */
        }
      }
      await load();
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusyId("");
    }
  }

  async function doCleanResidue() {
    if (!residue) return;
    setCleaningResidue(true);
    try {
      const r = await invoke<{ freed: number; errors: string[] }>("clean_residue", {
        paths: [residue.detail.path],
      });
      setMsg(
        r.errors.length > 0
          ? `残留清理：${r.errors.join("；")}`
          : `残留已清理，释放 ${formatBytes(r.freed)}。`,
      );
    } catch (e) {
      setMsg(String(e));
    } finally {
      setCleaningResidue(false);
      setResidue(null);
    }
  }

  const entries = scan?.entries ?? [];
  const active = entries.filter((e) => !e.trusted && !e.dismissed);
  const trusted = entries.filter((e) => e.trusted && !e.dismissed);
  const dismissed = entries.filter((e) => e.dismissed);

  /** 单个软件卡片。group 决定右下角的附加按钮。 */
  function card(e: BloatwareEntry, group: "active" | "trusted" | "dismissed") {
    return (
      <div key={e.id} className="rounded-2xl bg-[var(--color-surface)] px-4 py-3 shadow-[var(--shadow-card)]">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm font-semibold">{e.name}</span>
          {e.publisher && (
            <span className="text-xs text-[var(--color-text-secondary)]">{e.publisher}</span>
          )}
          {e.tags.map((t) => {
            const s = TAG_STYLE[t] ?? DEFAULT_TAG;
            return (
              <span
                key={t}
                className="rounded-full px-2 py-0.5 text-[10px] font-medium"
                style={{ background: s.bg, color: s.fg }}
              >
                {t}
              </span>
            );
          })}
          <span className="ml-auto text-sm font-bold text-[var(--color-primary-dark)]">
            {sizeText(e.sizeMb)}
          </span>
        </div>
        <ul className="mt-1.5 space-y-0.5 text-xs leading-relaxed text-[var(--color-text-secondary)]">
          {e.behaviors.map((b, i) => (
            <li key={i}>· {b}</li>
          ))}
        </ul>
        {group === "active" && e.suggestion && (
          <div className="mt-1 text-xs text-[var(--color-text-secondary)] opacity-80">{e.suggestion}</div>
        )}
        <div className="mt-2 flex flex-wrap items-center gap-2">
          {e.installLocation && (
            <button
              onClick={() => openInExplorer(e.installLocation, true)}
              className="flex items-center gap-1.5 rounded-lg border border-[var(--color-line)] px-3 py-1.5 text-xs font-medium text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary-dark)]"
            >
              <FolderOpen size={13} />
              打开位置
            </button>
          )}
          {e.uninstallable && (
            <button
              disabled={busyId === e.id}
              onClick={() => setConfirmTarget(e)}
              className="flex items-center gap-1.5 rounded-lg bg-[var(--color-primary)] px-3.5 py-1.5 text-xs font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)] disabled:opacity-50"
            >
              {busyId === e.id ? <Loader2 size={13} className="animate-spin" /> : <Trash2 size={13} />}
              {busyId === e.id ? "卸载中…" : "一键卸载"}
            </button>
          )}
          {group === "dismissed" ? (
            <button
              onClick={() => setIgnored(e, false)}
              className="flex items-center gap-1.5 rounded-lg border border-[var(--color-line)] px-3 py-1.5 text-xs font-medium text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-primary-dark)]"
            >
              <Bell size={13} />
              恢复提醒
            </button>
          ) : (
            <button
              onClick={() => setIgnored(e, true)}
              className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-primary-dark)]"
            >
              <BellOff size={13} />
              不再提醒
            </button>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-start justify-between px-8 pt-6 pb-2">
        <div>
          <h1 className="text-xl font-semibold">软件体检</h1>
          <p className="mt-0.5 text-sm text-[var(--color-text-secondary)]">
            看看哪些软件在开机自启、后台常驻或占空间——都是客观事实，卸不卸由你定
          </p>
        </div>
        {phase === "ready" && (
          <button
            onClick={load}
            className="flex items-center gap-1.5 rounded-xl border border-[var(--color-line)] bg-[var(--color-surface)] px-3.5 py-2 text-xs font-medium text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary-dark)]"
          >
            <RotateCcw size={14} />
            重新扫描
          </button>
        )}
      </header>

      {phase === "loading" && (
        <div className="flex flex-1 flex-col items-center justify-center gap-4">
          <Loader2 size={36} className="animate-spin text-[var(--color-primary)]" />
          <div className="text-sm text-[var(--color-text-secondary)]">正在盘点已安装软件…</div>
        </div>
      )}

      {phase === "ready" && (
        <div className="flex-1 overflow-y-auto px-8 pb-6 pt-2">
          {error && (
            <div className="mb-4 rounded-xl bg-[var(--color-surface)] px-4 py-3 text-sm text-[var(--color-caution)] shadow-[var(--shadow-card)]">
              {error}
            </div>
          )}
          {msg && (
            <div className="mb-4 rounded-xl bg-[var(--color-primary-soft)] px-4 py-3 text-sm">{msg}</div>
          )}

          {/* 人话结论 */}
          <div className="mb-5 rounded-2xl bg-[var(--color-primary-soft)] px-5 py-3.5 text-sm leading-relaxed">
            {active.length === 0 ? (
              "没有需要重点关注的软件，挺清爽。常用/系统软件已自动折叠在下方。"
            ) : (
              <>
                为你列出了 <b>{active.length}</b>{" "}
                个需要重点关注的软件（有开机自启、后台常驻或占用较大等行为）。以下均为客观陈述、
                <b>不代表软件有害</b>；是否保留由你判断，需要时可一键卸载——
                <b>我们绝不会替你自动删除任何东西</b>。
              </>
            )}
          </div>

          {/* 浏览器主页提示（扫描级中性说明） */}
          {scan && scan.browserNotes.length > 0 && (
            <div className="mb-5 rounded-2xl border border-[#FDE68A] bg-[#FFFBEB] p-4 shadow-[var(--shadow-card)]">
              <div className="flex items-center gap-2 text-sm font-semibold text-[#92400E]">
                <AlertTriangle size={16} />
                浏览器主页可能被更改
              </div>
              <ul className="mt-2 space-y-1 text-xs leading-relaxed text-[#92400E]">
                {scan.browserNotes.map((n, i) => (
                  <li key={i}>{n}</li>
                ))}
              </ul>
            </div>
          )}

          {/* 需重点关注 */}
          {active.length > 0 && (
            <>
              <div className="mb-2 text-sm font-semibold">需要关注的软件（按负担排序）</div>
              <div className="space-y-2">{active.map((e) => card(e, "active"))}</div>
            </>
          )}

          {/* 折叠：常用/系统软件 */}
          {trusted.length > 0 && (
            <div className="mt-5">
              <button
                onClick={() => setShowTrusted((v) => !v)}
                className="flex w-full items-center gap-1.5 text-sm font-semibold text-[var(--color-text-secondary)]"
              >
                {showTrusted ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
                常用 / 系统软件 · 通常无需处理（{trusted.length}）
              </button>
              {showTrusted && <div className="mt-2 space-y-2">{trusted.map((e) => card(e, "trusted"))}</div>}
            </div>
          )}

          {/* 折叠：已忽略 */}
          {dismissed.length > 0 && (
            <div className="mt-5">
              <button
                onClick={() => setShowDismissed((v) => !v)}
                className="flex w-full items-center gap-1.5 text-sm font-semibold text-[var(--color-text-secondary)]"
              >
                {showDismissed ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
                已忽略 · 你选择了不再提醒（{dismissed.length}）
              </button>
              {showDismissed && <div className="mt-2 space-y-2">{dismissed.map((e) => card(e, "dismissed"))}</div>}
            </div>
          )}

          <div className="mt-6 flex items-start gap-1.5 text-[11px] leading-relaxed text-[var(--color-text-secondary)]">
            <ShieldCheck size={13} className="mt-0.5 shrink-0" />
            <span>
              以上均为客观行为陈述，不代表软件有害。卸载走该软件自带的官方卸载程序，不直接删文件；
              一切操作都需你亲自确认。
            </span>
          </div>
        </div>
      )}

      {/* 卸载二次确认弹窗 */}
      <AnimatePresence>
        {confirmTarget && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="absolute inset-0 z-30 flex items-center justify-center bg-black/30 p-6"
            onClick={() => setConfirmTarget(null)}
          >
            <motion.div
              initial={{ scale: 0.94, y: 10 }}
              animate={{ scale: 1, y: 0 }}
              exit={{ scale: 0.94, y: 10 }}
              className="w-full max-w-md rounded-2xl bg-[var(--color-surface)] p-6 shadow-[var(--shadow-card-hover)]"
              onClick={(ev) => ev.stopPropagation()}
            >
              <div className="text-base font-semibold">卸载前，确认一下</div>
              <div className="mt-2 text-sm leading-relaxed text-[var(--color-text-secondary)]">
                即将运行 <b className="text-[var(--color-text-main)]">{confirmTarget.name}</b>{" "}
                自带的官方卸载程序。过程中可能会弹出该软件自己的卸载窗口，请按提示完成。
              </div>
              <div className="mt-3 rounded-xl bg-[var(--color-bg)] px-3.5 py-2.5 text-xs leading-relaxed text-[var(--color-text-secondary)]">
                我们不会替你删除任何文件，只是帮你调起它官方的卸载流程；卸载后可选择清理残留目录。
              </div>
              <div className="mt-4 flex gap-3">
                <button
                  onClick={() => setConfirmTarget(null)}
                  className="flex-1 rounded-xl border border-[var(--color-line)] py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]"
                >
                  再想想
                </button>
                <button
                  onClick={() => doUninstall(confirmTarget)}
                  className="flex-1 rounded-xl bg-[var(--color-primary)] py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                >
                  确认卸载
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* 卸载后残留清理提示 */}
      <AnimatePresence>
        {residue && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="absolute inset-0 z-30 flex items-center justify-center bg-black/30 p-6"
            onClick={() => setResidue(null)}
          >
            <motion.div
              initial={{ scale: 0.94, y: 10 }}
              animate={{ scale: 1, y: 0 }}
              exit={{ scale: 0.94, y: 10 }}
              className="w-full max-w-md rounded-2xl bg-[var(--color-surface)] p-6 shadow-[var(--shadow-card-hover)]"
              onClick={(ev) => ev.stopPropagation()}
            >
              <div className="flex items-center gap-2 text-base font-semibold">
                <CheckCircle2 size={18} className="text-[var(--color-safe)]" />
                已卸载，是否顺手清理残留？
              </div>
              <div className="mt-2 text-sm leading-relaxed text-[var(--color-text-secondary)]">
                「{residue.name}」卸载后，安装目录还残留约{" "}
                <b className="text-[var(--color-primary-dark)]">{formatBytes(residue.detail.size)}</b>：
              </div>
              <div className="mt-2 break-all rounded-xl bg-[var(--color-bg)] px-3.5 py-2 text-xs text-[var(--color-text-secondary)]">
                {residue.detail.path}
              </div>
              <div className="mt-4 flex gap-3">
                <button
                  onClick={() => setResidue(null)}
                  className="flex-1 rounded-xl border border-[var(--color-line)] py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]"
                >
                  先留着
                </button>
                <button
                  disabled={cleaningResidue}
                  onClick={doCleanResidue}
                  className="flex-1 rounded-xl bg-[var(--color-primary)] py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)] disabled:opacity-50"
                >
                  {cleaningResidue ? "清理中…" : "清理残留"}
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
