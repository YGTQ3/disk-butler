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
  Box,
  Power,
  Flame,
  X,
  ShieldAlert,
  Check,
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
  icon?: string | null;
  tags: string[];
  behaviors: string[];
  suggestion: string;
  trusted: boolean;
  security: boolean;
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

/** 卸载后残留全景报告：文件目录 + 当前用户注册表键 */
interface ResidueScanReport {
  dirs: ResidueDetail[];
  regKeys: string[];
}

/** 强力卸载预览：将清除的内容清单 */
interface ForcePlan {
  name: string;
  installDir: string;
  dirDeletable: boolean;
  services: string[];
  tasks: string[];
  regPath: string;
}

type UninStatus = "confirm" | "auth" | "running" | "done" | "fail";
/** 卸载/停止任务（驱动进度弹窗） */
interface UninJob {
  entry: BloatwareEntry;
  mode: "normal" | "force" | "stop";
  status: UninStatus;
  step: number;
  detail: string;
  plan?: ForcePlan;
}
const NORMAL_STEPS = ["准备", "授权", "卸载中", "验证"];
const FORCE_STEPS = ["准备", "授权", "清后台", "删文件", "验证"];
const STOP_STEPS = ["准备", "授权", "停后台", "完成"];

type Phase = "loading" | "ready";

const TAG_STYLE: Record<string, { bg: string; fg: string }> = {
  开机自启: { bg: "var(--color-tag-indigo-bg)", fg: "var(--color-tag-indigo-text)" },
  后台常驻: { bg: "var(--color-tag-cyan-bg)", fg: "var(--color-tag-cyan-text)" },
  占用较大: { bg: "var(--color-tag-bg)", fg: "var(--color-tag-text)" },
};
const DEFAULT_TAG = { bg: "var(--color-tag-bg)", fg: "var(--color-tag-text)" };

/** MB 数 -> 人话大小；null 显示占位 */
function sizeText(mb: number | null): string {
  if (mb == null || mb <= 0) return "—";
  return formatBytes(mb * 1024 * 1024);
}

export default function BloatwareCheck() {
  const [phase, setPhase] = useState<Phase>("loading");
  const [scan, setScan] = useState<BloatwareScan | null>(null);
  const [error, setError] = useState("");

  // 结果/错误通知：一律弹独立窗口，不在页面里内联平铺
  const [notice, setNotice] = useState<{ kind: "error" | "success"; text: string } | null>(null);
  // 卸载久未完成时的顶部提示：卸载器向导多为居中弹出，会盖住居中的进度弹窗，
  // 故把引导另放到页面顶部边缘，确保用户能看到。
  const [uninHint, setUninHint] = useState(false);
  // 卸载任务（进度弹窗）：confirm → running → done/fail
  const [job, setJob] = useState<UninJob | null>(null);
  // 安全软件卸载引导弹窗（有自我保护，无法静默卸载，只能引导用户走官方流程）
  const [guide, setGuide] = useState<BloatwareEntry | null>(null);

  // 卸载后残留清单（Geek 式：目录+注册表键，勾选确认后清除）
  const [residue, setResidue] = useState<{
    entry: BloatwareEntry;
    report: ResidueScanReport;
    checkedDirs: Set<string>;
    checkedRegs: Set<string>;
  } | null>(null);
  const [cleaningResidue, setCleaningResidue] = useState(false);

  // AppData 孤儿残留检查已迁至「一键清理」页

  // 折叠区展开状态
  const [showTrusted, setShowTrusted] = useState(false);
  const [showDismissed, setShowDismissed] = useState(false);
  // 视图模式：false=只看需关注（默认，快）；true=全部已装软件（按需重扫）
  const [showAll, setShowAll] = useState(false);

  async function load(all: boolean) {
    setPhase("loading");
    setError("");
    try {
      setScan(await invoke<BloatwareScan>("scan_bloatware", { includeAll: all }));
      setShowAll(all);
    } catch (e) {
      setError(String(e));
    }
    setPhase("ready");
  }

  useEffect(() => {
    load(false);
  }, []);

  /** 记录/取消"不再提醒" */
  async function setIgnored(e: BloatwareEntry, ignored: boolean) {
    try {
      await invoke("bloatware_set_ignored", { key: e.key, ignored });
      await load(showAll);
    } catch (err) {
      setNotice({ kind: "error", text: String(err) });
    }
  }

  /** 打开「停止后台运行」确认弹窗（mode=stop）；提权停服务+进程，二次确认+进度均在弹窗内 */
  async function stopSoftware(e: BloatwareEntry) {
    setJob({ entry: e, mode: "stop", status: "confirm", step: 0, detail: "正在读取将处理的内容…" });
    try {
      const plan = await invoke<ForcePlan>("bloatware_force_preview", { id: e.id });
      setJob((j) => (j && j.entry.id === e.id ? { ...j, plan, detail: "" } : j));
    } catch {
      setJob((j) => (j && j.entry.id === e.id ? { ...j, detail: "" } : j));
    }
  }

  /** 打开普通卸载（方法A）确认弹窗，并拉取将处理的清单（服务/进程） */
  async function startUninstall(e: BloatwareEntry) {
    setJob({ entry: e, mode: "normal", status: "confirm", step: 0, detail: "正在读取将处理的内容…" });
    try {
      const plan = await invoke<ForcePlan>("bloatware_force_preview", { id: e.id });
      setJob((j) => (j && j.entry.id === e.id ? { ...j, plan, detail: "" } : j));
    } catch {
      setJob((j) => (j && j.entry.id === e.id ? { ...j, detail: "" } : j));
    }
  }

  /** 打开强力卸载（方法C）确认弹窗，并异步拉取将清除内容预览 */
  async function startForce(e: BloatwareEntry) {
    setJob({ entry: e, mode: "force", status: "confirm", step: 0, detail: "正在读取将清除的内容…" });
    try {
      const plan = await invoke<ForcePlan>("bloatware_force_preview", { id: e.id });
      setJob((j) => (j && j.entry.id === e.id ? { ...j, plan, detail: "" } : j));
    } catch (err) {
      setJob((j) => (j && j.entry.id === e.id ? { ...j, status: "fail", detail: String(err) } : j));
    }
  }

  /** 执行卸载/停止：置顶 → 授权（等你在系统窗点“是”）→ 授权通过后才进入执行 → 验证 */
  async function runJob() {
    const j = job;
    if (!j) return;
    const steps = j.mode === "force" ? FORCE_STEPS : j.mode === "stop" ? STOP_STEPS : NORMAL_STEPS;
    // 不再把主窗置顶：能静默卸载的软件根本不弹窗，置顶无意义；不能静默的会弹卸载向导，
    // 置顶反而会盖住它导致卡死。索性不置顶，让向导窗口正常显示在前台。
    setUninHint(false);
    setJob({ ...j, status: "running", step: 1, detail: "请在弹出的系统授权窗口点「是」，点了我们就立刻开始…" });

    const cmd = j.mode === "force" ? "force_uninstall_software" : j.mode === "stop" ? "stop_software" : "uninstall_software";
    // 三种模式统一传 id：后端据 id 重读安装目录/卸载命令，不接收前端直传路径（安全信任边界）
    const args = { id: j.entry.id };
    const p = invoke(cmd, args);

    // 轮询：只有当提权脚本真正开始执行（你已点“是”）后，才推进到“执行中”
    let advanced = false;
    let settled = false;
    // 宽限计时器：不能静默卸载的软件会弹出自己的卸载向导等用户点击。若超时仍未结束，
    // 判定为“卸载器在等你操作”——更新引导文案说明进度为何停住。后台仍在等待，点完自动继续。
    let graceTimer: ReturnType<typeof setTimeout> | undefined;
    const poll = setInterval(async () => {
      if (advanced) return;
      try {
        if (await invoke<boolean>("op_started")) {
          advanced = true;
          setJob((cur) => (cur ? { ...cur, step: 2, detail: "正在处理，请稍等，马上就好…" } : cur));
          if (j.mode !== "stop") {
            graceTimer = setTimeout(() => {
              if (settled) return;
              setUninHint(true);
              setJob((cur) =>
                cur && !settled
                  ? {
                      ...cur,
                      detail:
                        "如果屏幕上弹出了这个软件自己的卸载向导窗口，请在它上面完成操作（点“下一步/卸载/是”）。完成后这里会自动继续，不用管本窗口。",
                    }
                  : cur,
              );
            }, 7000);
          }
        }
      } catch {
        /* 忽略轮询错误 */
      }
    }, 250);

    try {
      await p;
      settled = true;
      clearInterval(poll);
      if (graceTimer) clearTimeout(graceTimer);
      setUninHint(false);
      setJob((cur) => (cur ? { ...cur, step: steps.length - 1 } : cur));
      if (j.mode === "stop") {
        setScan((prev) =>
          prev
            ? {
                ...prev,
                entries: prev.entries.map((x) =>
                  x.key === j.entry.key
                    ? {
                        ...x,
                        residentMemMb: 0,
                        tags: x.tags.filter((t) => t !== "后台常驻"),
                        behaviors: x.behaviors.filter((b) => !b.includes("后台进程")),
                      }
                    : x,
                ),
              }
            : prev,
        );
        setJob((cur) =>
          cur ? { ...cur, status: "done", step: steps.length, detail: `已停止「${j.entry.name}」的后台服务与进程。` } : cur,
        );
        return;
      }
      if (j.mode === "normal" || j.mode === "force") {
        try {
          const report = await invoke<ResidueScanReport>("scan_residue", {
            name: j.entry.name,
            publisher: j.entry.publisher,
            installLocation: j.entry.installLocation,
          });
          if (report.dirs.length > 0 || report.regKeys.length > 0) {
            setResidue({
              entry: j.entry,
              report,
              checkedDirs: new Set(report.dirs.map((d) => d.path)),
              checkedRegs: new Set(report.regKeys),
            });
          }
        } catch {
          /* 残留探测失败不打扰 */
        }
      }
      setJob((cur) =>
        cur ? { ...cur, status: "done", step: steps.length, detail: `「${j.entry.name}」已成功卸载。` } : cur,
      );
    } catch (err) {
      settled = true;
      clearInterval(poll);
      if (graceTimer) clearTimeout(graceTimer);
      setUninHint(false);
      setJob((cur) => (cur ? { ...cur, status: "fail", detail: String(err) } : cur));
    }
  }

  /** 关闭进度弹窗：卸载完成则刷新列表（停止后台不刷新，保留条目） */
  async function closeJob() {
    setUninHint(false);
    const j = job;
    setJob(null);
    if (j?.status === "done" && j.mode !== "stop") await load(showAll);
  }

  async function doCleanResidue() {
    if (!residue) return;
    setCleaningResidue(true);
    try {
      const r = await invoke<{ freed: number; errors: string[] }>("clean_residue", {
        name: residue.entry.name,
        publisher: residue.entry.publisher,
        installLocation: residue.entry.installLocation,
        dirs: [...residue.checkedDirs],
        regKeys: [...residue.checkedRegs],
      });
      setNotice(
        r.errors.length > 0
          ? { kind: "error", text: `残留清理：${r.errors.join("；")}` }
          : { kind: "success", text: `残留已清理${r.freed > 0 ? `，释放 ${formatBytes(r.freed)}` : ""}。` },
      );
    } catch (e) {
      setNotice({ kind: "error", text: String(e) });
    } finally {
      setCleaningResidue(false);
      setResidue(null);
    }
  }

  /** 清理勾选的孤儿目录（已迁至一键清理页，此处移除） */

  const entries = scan?.entries ?? [];
  const security = entries.filter((e) => e.security && !e.dismissed);
  const active = entries.filter((e) => !e.security && !e.trusted && !e.dismissed);
  const trusted = entries.filter((e) => e.trusted && !e.security && !e.dismissed);
  const dismissed = entries.filter((e) => e.dismissed);

  /** 单个软件卡片。group 决定右下角的附加按钮。 */
  function card(e: BloatwareEntry, group: "active" | "trusted" | "dismissed") {
    return (
      <div key={e.id} className="rounded-2xl bg-[var(--color-surface)] px-4 py-3 shadow-[var(--shadow-card)]">
        <div className="flex flex-wrap items-center gap-2">
          {e.icon ? (
            <img src={e.icon} alt="" className="h-5 w-5 shrink-0 rounded-sm" />
          ) : (
            <Box size={18} className="shrink-0 text-[var(--color-text-secondary)] opacity-40" />
          )}
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
          {!e.security && e.residentMemMb > 0 && (
            <button
              onClick={() => stopSoftware(e)}
              className="flex items-center gap-1.5 rounded-lg border border-warn-border bg-warn-bg px-3 py-1.5 text-xs font-medium text-warn-text transition-colors hover:bg-warn-bg-strong"
            >
              <Power size={13} />
              停止后台运行
            </button>
          )}
          {e.security ? (
            <button
              onClick={() => setGuide(e)}
              className="flex items-center gap-1.5 rounded-lg bg-[var(--color-primary)] px-3.5 py-1.5 text-xs font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)]"
            >
              <ShieldAlert size={13} />
              卸载（按步骤）
            </button>
          ) : (
            <>
              {e.uninstallable && (
                <button
                  onClick={() => startUninstall(e)}
                  className="flex items-center gap-1.5 rounded-lg bg-[var(--color-primary)] px-3.5 py-1.5 text-xs font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                >
                  <Trash2 size={13} />
                  一键卸载
                </button>
              )}
              {!!e.installLocation && (
                <button
                  onClick={() => startForce(e)}
                  className="flex items-center gap-1.5 rounded-lg border border-danger-border bg-danger-bg-soft px-3 py-1.5 text-xs font-medium text-danger-dark transition-colors hover:bg-danger-bg"
                >
                  <Flame size={13} />
                  强力卸载
                </button>
              )}
              {!e.uninstallable && !e.installLocation && (
                <button
                  onClick={() => invoke("open_apps_settings")}
                  className="flex items-center gap-1.5 rounded-lg border border-[var(--color-line)] px-3 py-1.5 text-xs font-medium text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary-dark)]"
                >
                  <Trash2 size={13} />
                  去系统卸载
                </button>
              )}
            </>
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

  /** 明细清单的一行（标签 + 条目列表） */
  function planRow(label: string, items: string[], empty: string) {
    return (
      <div className="rounded-xl bg-[var(--color-bg)] px-4 py-2.5">
        <div className="mb-1 text-xs font-semibold text-[var(--color-text-secondary)]">{label}</div>
        {items.length > 0 ? (
          <ul className="space-y-0.5">
            {items.map((it, i) => (
              <li key={i} className="break-all text-[13px] text-[var(--color-text-main)]">{it}</li>
            ))}
          </ul>
        ) : (
          <div className="text-[13px] text-[var(--color-text-secondary)] opacity-70">{empty}</div>
        )}
      </div>
    );
  }

  /** 16:9 居中圆角卸载/停止进度弹窗（confirm / running / done / fail） */
  function jobModal() {
    if (!job) return null;
    const isForce = job.mode === "force";
    const isStop = job.mode === "stop";
    const steps = isForce ? FORCE_STEPS : isStop ? STOP_STEPS : NORMAL_STEPS;
    const headerTitle = isForce ? "强力卸载（高级）" : isStop ? "停止后台运行" : "卸载软件";
    const actionWord = isForce ? "强力卸载" : isStop ? "停止后台运行" : "卸载";
    const accent = isForce ? "var(--color-danger-dark)" : isStop ? "var(--color-warn-text-strong)" : "var(--color-primary-dark)";
    // 完成后告知用户「后果」——它现在处于什么状态、之后会怎样
    const doneNote = isStop
      ? "它现在不再占用内存运行了。下次开机、或你手动打开它时，它可能会再次启动——如果想让它彻底别再自启，可以直接把它卸载掉。"
      : isForce
        ? "它的安装文件夹、后台服务、定时任务和注册表记录都已尽力清除干净。个别正被占用的文件，可能要重启电脑后才会彻底消失。接着我们会再帮你查一遍有没有其他残留的文件夹和设置项。"
        : "它已经从你的电脑上移除了。接着我们会帮你查一遍有没有残留的文件夹和注册表设置，你可以挑选着一起清掉。";
    // 确认页：每步 = 标题(可空,避免与框重复) + 明细框 + 是否并排
    const svc = job.plan?.services ?? [];
    const tsk = job.plan?.tasks ?? [];
    const reg = job.plan?.regPath ? [job.plan.regPath] : [];
    const dir = job.plan?.dirDeletable && job.plan.installDir ? [job.plan.installDir] : [];
    const dirEmpty = job.plan?.installDir ? "（不在常规安装区，为安全起见不删除）" : "（未定位到安装文件夹）";
    const resident = job.entry.residentMemMb > 0 ? [`当前约 ${job.entry.residentMemMb} MB，将一并结束`] : [];
    type Row = { label: string; items: string[]; empty: string };
    type Step = { title: string | null; rows: Row[]; side?: boolean };
    const confirmSteps: Step[] = isStop
      ? [
          { title: "关掉它注册在后台的服务", rows: [{ label: "将停止的服务", items: svc, empty: "（无）" }] },
          { title: "结束它正在运行的后台进程，让它别再占内存", rows: [{ label: "后台进程", items: resident, empty: "（当前无）" }] },
        ]
      : isForce
        ? [
            {
              title: "强制关闭并删除它的后台服务、定时任务和进程",
              side: true,
              rows: [
                { label: "服务", items: svc, empty: "（无）" },
                { label: "计划任务", items: tsk, empty: "（无）" },
                { label: "后台进程", items: resident, empty: "（当前无）" },
              ],
            },
            { title: null, rows: [{ label: "卸载方式", items: ["用软件自带的卸载程序帮你卸掉"], empty: "" }] },
            {
              title: "删掉安装文件夹和相关注册表记录，清得更彻底",
              side: true,
              rows: [
                { label: "安装文件夹", items: dir, empty: dirEmpty },
                { label: "注册表记录", items: reg, empty: "（无）" },
              ],
            },
          ]
        : [
            {
              title: "先关掉它在后台偷偷运行的服务和进程，免得挡住卸载",
              side: true,
              rows: [
                { label: "将停止的服务", items: svc, empty: "（无）" },
                { label: "后台进程", items: resident, empty: "（当前无）" },
              ],
            },
            { title: null, rows: [{ label: "卸载方式", items: ["用软件自带的卸载程序帮你卸掉，不直接删文件"], empty: "" }] },
            { title: "检查有没有卸干净，还能顺手清掉残留的文件夹", rows: [] },
          ];
    return (
      <motion.div
        key="jobmodal"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="absolute inset-0 z-40 flex items-center justify-center bg-black/40 p-6"
      >
        <motion.div
          initial={{ scale: 0.95, y: 12 }}
          animate={{ scale: 1, y: 0 }}
          exit={{ scale: 0.95, y: 12 }}
          className="flex max-h-[86vh] w-[min(92%,660px)] flex-col overflow-hidden rounded-2xl bg-[var(--color-surface)] shadow-[var(--shadow-card-hover)]"
        >
          {/* 头部 */}
          <div className="flex items-center gap-3 px-7 pt-6 pb-4">
            {isForce ? (
              <Flame size={24} className="text-danger-dark" />
            ) : isStop ? (
              <Power size={24} className="text-warn-text-strong" />
            ) : (
              <Trash2 size={24} className="text-[var(--color-primary)]" />
            )}
            <div className="min-w-0">
              <div className="text-lg font-semibold">{headerTitle}</div>
              <div className="truncate text-sm text-[var(--color-text-secondary)]">{job.entry.name}</div>
            </div>
            {(job.status === "done" || job.status === "fail") && (
              <button onClick={closeJob} className="ml-auto rounded-lg p-1.5 text-[var(--color-text-secondary)] hover:bg-[var(--color-bg)]">
                <X size={20} />
              </button>
            )}
          </div>

          {/* 主体 */}
          <div className="flex-1 overflow-auto px-7 py-2">
            {job.status === "confirm" && (
              <div className="mx-auto flex w-full max-w-[500px] flex-col gap-4 pb-1">
                {/* 主标题：软件图标 + 即将XX + 名称（居中放大，为最主要内容） */}
                <div className="flex flex-col items-center gap-2 pt-2 text-center">
                  {job.entry.icon ? (
                    <img src={job.entry.icon} alt="" className="h-12 w-12 rounded-xl" />
                  ) : (
                    <Box size={40} className="text-[var(--color-text-secondary)] opacity-40" />
                  )}
                  <div className="text-sm text-[var(--color-text-secondary)]">即将{actionWord}</div>
                  <div className="text-2xl font-bold" style={{ color: accent }}>{job.entry.name}</div>
                </div>

                {isForce && (
                  <div className="flex items-center justify-center gap-1.5 text-sm font-semibold text-danger-dark">
                    <ShieldAlert size={16} />
                    第 3 步会删除文件，且不可撤销
                  </div>
                )}

                {/* 我们将做什么 —— 每步序号 + 对应明细框 */}
                <div>
                  <div className="mb-2 text-sm font-semibold">我们将做什么</div>
                  {!job.plan ? (
                    <div className="flex items-center gap-2 text-sm text-[var(--color-text-secondary)]">
                      <Loader2 size={16} className="animate-spin" />
                      {job.detail || "正在读取将处理的内容…"}
                    </div>
                  ) : (
                    <div className="space-y-3">
                      {confirmSteps.map((st, i) => (
                        <div key={i} className="flex gap-2.5">
                          <span
                            className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-xs font-bold text-white"
                            style={{ background: accent }}
                          >
                            {i + 1}
                          </span>
                          <div className="flex-1 space-y-1.5">
                            {st.title && (
                              <div className="pt-0.5 text-sm font-medium leading-relaxed text-[var(--color-text-main)]">{st.title}</div>
                            )}
                            {st.rows.length > 0 && (
                              <div className={st.side && st.rows.length > 1 ? "grid grid-cols-2 gap-2" : "space-y-1.5"}>
                                {st.rows.map((r, j) => (
                                  <div key={j}>{planRow(r.label, r.items, r.empty)}</div>
                                ))}
                              </div>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            )}

            {job.status === "auth" && (
              <div className="mx-auto flex h-full max-w-[440px] flex-col items-center justify-center gap-4 py-8 text-center">
                <div
                  className="flex h-14 w-14 items-center justify-center rounded-full"
                  style={{ background: isForce ? "var(--color-danger-bg-soft)" : "var(--color-primary-soft)" }}
                >
                  <ShieldAlert size={28} style={{ color: accent }} />
                </div>
                <div className="text-lg font-bold" style={{ color: accent }}>
                  确定要{actionWord}「{job.entry.name}」吗？
                </div>
                <div className="text-[15px] leading-relaxed text-[var(--color-text-secondary)]">
                  点「我知道了，继续」后，屏幕会弹出一个<b className="text-[var(--color-text-main)]">系统授权小窗</b>——点「是」就行。剩下的全交给我们，你不用再动手。
                </div>
              </div>
            )}

            {(job.status === "running" || job.status === "done" || job.status === "fail") && (
              <div className="flex flex-col gap-6 py-5">
                {/* 横向步骤条：一 — 二 — 三 — 四 排在一条线上 */}
                <div className="flex items-start">
                  {steps.map((s, i) => {
                    const done = job.step > i;
                    const current = job.step === i && job.status === "running";
                    const active = done || current;
                    const running = job.status === "running";
                    // 连接线：左半段(前一步→本步)已达则填充；右半段(本步→下一步)已过则填充
                    const leftFilled = job.step >= i;
                    const rightFilled = job.step > i;
                    const seg = (filled: boolean, hide: boolean) => (
                      <div className="relative h-1.5 flex-1 overflow-hidden rounded-full" style={{ background: hide ? "transparent" : filled ? accent : "var(--color-line)" }}>
                        {filled && !hide && running && <span className="step-shine" />}
                      </div>
                    );
                    return (
                      <div key={i} className="flex flex-1 flex-col items-center">
                        <div className="flex w-full items-center">
                          {seg(leftFilled, i === 0)}
                          <div className="relative z-10 -mx-1 shrink-0">
                            {/* 当前阶段：外圈转圈 */}
                            {current && (
                              <span
                                className="absolute -inset-1.5 animate-spin rounded-full border-2 border-transparent"
                                style={{ borderTopColor: accent, borderRightColor: accent }}
                              />
                            )}
                            <div
                              className="flex h-10 w-10 items-center justify-center rounded-full text-sm font-bold transition-colors"
                              style={
                                active
                                  ? { background: accent, color: "#fff" }
                                  : { background: "var(--color-bg)", color: "var(--color-text-secondary)" }
                              }
                            >
                              {done ? <Check size={18} /> : i + 1}
                            </div>
                          </div>
                          {seg(rightFilled, i === steps.length - 1)}
                        </div>
                        <div
                          className={
                            "mt-2.5 text-center text-xs leading-tight " +
                            (current ? "font-semibold text-[var(--color-text-main)]" : "text-[var(--color-text-secondary)]")
                          }
                        >
                          {s}
                        </div>
                      </div>
                    );
                  })}
                </div>

                {/* 状态详情 */}
                <div className="text-center text-[15px]">
                  {job.status === "done" && (
                    <div>
                      <div className="flex items-center justify-center gap-2 font-semibold text-[var(--color-safe)]">
                        <CheckCircle2 size={20} />
                        {job.detail}
                      </div>
                      <div className="mx-auto mt-2 max-w-[460px] text-sm leading-relaxed text-[var(--color-text-secondary)]">
                        {doneNote}
                      </div>
                    </div>
                  )}
                  {job.status === "running" && (
                    <div className="text-sm text-[var(--color-text-secondary)]">正在处理，请稍等，马上就好…</div>
                  )}
                  {job.status === "fail" && (
                    <div className="rounded-xl bg-danger-bg-soft px-4 py-3 text-left text-sm leading-relaxed text-danger-text">
                      {job.detail}
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>

          {/* 底部按钮 */}
          <div className="px-6 pb-5 pt-2">
            {job.status === "confirm" && job.mode === "normal" && (
              <div className="flex items-center gap-3">
                <button
                  onClick={() => startForce(job.entry)}
                  className="mr-auto flex items-center gap-1.5 rounded-xl bg-danger px-4 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-danger-dark"
                >
                  <Flame size={15} />
                  改用强力卸载
                </button>
                <button onClick={closeJob} className="rounded-xl border border-[var(--color-line)] px-5 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]">
                  取消
                </button>
                <button onClick={() => setJob({ ...job, status: "auth" })} className="rounded-xl bg-[var(--color-primary)] px-6 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-[var(--color-primary-dark)]">
                  开始卸载
                </button>
              </div>
            )}
            {job.status === "confirm" && job.mode === "stop" && (
              <div className="flex items-center justify-end gap-3">
                <button onClick={closeJob} className="rounded-xl border border-[var(--color-line)] px-5 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]">
                  取消
                </button>
                <button onClick={() => setJob({ ...job, status: "auth" })} className="rounded-xl bg-warn-icon px-6 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-warn-text-strong">
                  确认停止后台
                </button>
              </div>
            )}
            {job.status === "confirm" && isForce && (
              <div className="flex items-center justify-end gap-3">
                <button onClick={closeJob} className="rounded-xl border border-[var(--color-line)] px-5 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]">
                  取消
                </button>
                <button
                  disabled={!job.plan}
                  onClick={() => setJob({ ...job, status: "auth" })}
                  className="rounded-xl bg-danger px-6 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-danger-dark disabled:opacity-50"
                >
                  我已确认，强力卸载
                </button>
              </div>
            )}
            {job.status === "auth" && (
              <div className="flex items-center justify-end gap-3">
                <button
                  onClick={() => setJob({ ...job, status: "confirm" })}
                  className="rounded-xl border border-[var(--color-line)] px-5 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]"
                >
                  再想想
                </button>
                <button
                  onClick={runJob}
                  className="rounded-xl px-6 py-2.5 text-sm font-semibold text-white transition-opacity hover:opacity-90"
                  style={{ background: accent }}
                >
                  我知道了，继续
                </button>
              </div>
            )}
            {job.status === "done" && (
              <div className="flex justify-end">
                <button onClick={closeJob} className="rounded-xl bg-[var(--color-primary)] px-6 py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)]">
                  返回列表
                </button>
              </div>
            )}
            {job.status === "fail" && (
              <div className="flex items-center justify-end gap-3">
                {job.mode === "normal" && (
                  <button
                    onClick={() => startForce(job.entry)}
                    className="mr-auto flex items-center gap-1.5 rounded-xl bg-danger px-4 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-danger-dark"
                  >
                    <Flame size={15} />
                    改用强力卸载
                  </button>
                )}
                <button onClick={closeJob} className="rounded-xl border border-[var(--color-line)] px-5 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]">
                  关闭
                </button>
              </div>
            )}
          </div>
        </motion.div>
      </motion.div>
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
          <div className="flex items-center gap-2">
            <div className="flex rounded-xl border border-[var(--color-line)] p-0.5 text-xs font-medium">
              <button
                onClick={() => { if (showAll) load(false); }}
                className={[
                  "rounded-lg px-3 py-1.5 transition-colors",
                  !showAll ? "bg-[var(--color-primary-soft)] text-[var(--color-primary-dark)]" : "text-[var(--color-text-secondary)]",
                ].join(" ")}
              >
                需关注
              </button>
              <button
                onClick={() => { if (!showAll) load(true); }}
                className={[
                  "rounded-lg px-3 py-1.5 transition-colors",
                  showAll ? "bg-[var(--color-primary-soft)] text-[var(--color-primary-dark)]" : "text-[var(--color-text-secondary)]",
                ].join(" ")}
              >
                全部软件
              </button>
            </div>
            <button
              onClick={() => load(showAll)}
              className="flex items-center gap-1.5 rounded-xl border border-[var(--color-line)] bg-[var(--color-surface)] px-3.5 py-2 text-xs font-medium text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary-dark)]"
            >
              <RotateCcw size={14} />
              重新扫描
            </button>
          </div>
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

          {/* 人话结论 */}
          <div className="mb-5 rounded-2xl bg-[var(--color-primary-soft)] px-5 py-3.5 text-sm leading-relaxed">
            {showAll ? (
              <>
                已列出<b>全部 {active.length}</b> 个软件（常用/系统与已忽略的已折叠在下方）。均为客观陈述、
                <b>不代表软件有害</b>；可按需打开位置或卸载——<b>绝不会替你自动删除任何东西</b>。
              </>
            ) : active.length === 0 ? (
              "没有需要重点关注的软件，挺清爽。切到右上角「全部软件」可管理所有已装软件。"
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
            <div className="mb-5 rounded-2xl border border-warn-border bg-warn-bg p-4 shadow-[var(--shadow-card)]">
              <div className="flex items-center gap-2 text-sm font-semibold text-warn-text">
                <AlertTriangle size={16} />
                浏览器主页可能被更改
              </div>
              <ul className="mt-2 space-y-1 text-xs leading-relaxed text-warn-text">
                {scan.browserNotes.map((n, i) => (
                  <li key={i}>{n}</li>
                ))}
              </ul>
            </div>
          )}

          {/* 多杀软共存提醒 */}
          {security.length >= 2 && (
            <div className="mb-4 rounded-2xl border border-danger-border bg-danger-bg-soft p-4 shadow-[var(--shadow-card)]">
              <div className="flex items-center gap-2 text-sm font-semibold text-danger-dark">
                <ShieldAlert size={16} />
                检测到 {security.length} 款安全 / 杀毒软件同时安装
              </div>
              <div className="mt-1.5 text-xs leading-relaxed text-danger-text">
                多个杀毒软件共存通常会<b>互相冲突</b>：抢占系统进程、拖慢开机与日常运行、占用大量内存，甚至互相误判查杀。
                <b>建议只保留 1 款</b>就够了，其余可以卸载；保留 0 款也行——Windows 自带的 Defender 已提供基础防护。
              </div>
            </div>
          )}

          {/* 安全软件 / 杀毒软件（单独归类） */}
          {security.length > 0 && (
            <>
              <div className="mb-2 flex items-center gap-1.5 text-sm font-semibold">
                <ShieldAlert size={15} className="text-[var(--color-primary-dark)]" />
                安全软件 / 杀毒软件（{security.length}）
              </div>
              <div className="space-y-2">{security.map((e) => card(e, "active"))}</div>
              <div className="mb-5" />
            </>
          )}

          {/* 需重点关注 */}
          {active.length > 0 && (
            <>
              <div className="mb-2 text-sm font-semibold">
                {showAll ? "全部软件（常用/系统已折叠在下方）" : "需要关注的软件（按负担排序）"}
              </div>
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

      {/* 卸载进度弹窗（16:9 居中圆角，全程只在此交互，卸载期间主窗口置顶） */}
      <AnimatePresence>{jobModal()}</AnimatePresence>

      {/* 安全软件卸载引导弹窗（自我保护无法静默卸载，引导用户走官方流程） */}
      <AnimatePresence>
        {guide && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="absolute inset-0 z-40 flex items-center justify-center bg-black/40 p-6"
            onClick={() => setGuide(null)}
          >
            <motion.div
              initial={{ scale: 0.95, y: 12 }}
              animate={{ scale: 1, y: 0 }}
              exit={{ scale: 0.95, y: 12 }}
              className="flex max-h-[86vh] w-[min(92%,620px)] flex-col overflow-hidden rounded-2xl bg-[var(--color-surface)] shadow-[var(--shadow-card-hover)]"
              onClick={(ev) => ev.stopPropagation()}
            >
              <div className="flex items-center gap-3 px-7 pt-6 pb-3">
                <ShieldAlert size={24} className="text-[var(--color-primary-dark)]" />
                <div className="min-w-0">
                  <div className="text-lg font-semibold">卸载安全软件</div>
                  <div className="truncate text-sm text-[var(--color-text-secondary)]">{guide.name}</div>
                </div>
                <button onClick={() => setGuide(null)} className="ml-auto rounded-lg p-1.5 text-[var(--color-text-secondary)] hover:bg-[var(--color-bg)]">
                  <X size={20} />
                </button>
              </div>

              <div className="flex-1 overflow-auto px-7 py-2">
                <div className="rounded-xl bg-danger-bg-soft px-4 py-3 text-sm leading-relaxed text-danger-text">
                  <b>{guide.name}</b> 是安全 / 杀毒软件，带<b>内核级自我保护</b>——任何第三方（包括我们）都<b>无法替你静默停止或卸载</b>。这是杀软防止被恶意程序卸载的正常防护，<b>不是故障</b>。所以要你亲自在它自己的流程里完成。
                </div>

                <div className="mt-4 mb-2 text-sm font-semibold">按这几步做</div>
                <ol className="space-y-3">
                  {[
                    "点下面「打开它自带的卸载程序」，会弹出它自己的卸载界面（可能需要授权、可能有挽留或广告页，属正常）。",
                    "按它的提示一步步走完；如果它要求先「关闭自我保护 / 退出防护」，先照做再继续卸载。",
                    "若卸载被拦截或失败：重启电脑 → 开机时进入 Windows「安全模式」（自我保护不加载）→ 再卸载一次通常就能成功。",
                    "卸载完回到这里点右上角「重新扫描」，确认它是否已消失。",
                  ].map((s, i) => (
                    <li key={i} className="flex items-start gap-2.5 text-sm leading-relaxed">
                      <span className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-[var(--color-primary-dark)] text-xs font-bold text-white">
                        {i + 1}
                      </span>
                      <span>{s}</span>
                    </li>
                  ))}
                </ol>

                <div className="mt-4 rounded-xl bg-[var(--color-bg)] px-4 py-3 text-[13px] leading-relaxed text-[var(--color-text-secondary)]">
                  提醒：卸载杀毒软件会降低电脑防护。若你打算不再装同类软件，Windows 自带的 Defender 会自动接管基础防护；若装了多款，建议只留 1 款。
                </div>
              </div>

              <div className="flex items-center justify-end gap-3 px-6 pb-5 pt-2">
                <button
                  onClick={() => setGuide(null)}
                  className="rounded-xl border border-[var(--color-line)] px-5 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]"
                >
                  关闭
                </button>
                <button
                  onClick={() => {
                    invoke("open_official_uninstaller", { id: guide.id }).catch((e) =>
                      setNotice({ kind: "error", text: String(e) }),
                    );
                    setGuide(null);
                  }}
                  className="rounded-xl bg-[var(--color-primary)] px-6 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                >
                  打开它自带的卸载程序
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* 卸载后残留清理清单（Geek 式：勾选目录/注册表键后清除） */}
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
              className="flex max-h-[80%] w-full max-w-lg flex-col rounded-2xl bg-[var(--color-surface)] p-6 shadow-[var(--shadow-card-hover)]"
              onClick={(ev) => ev.stopPropagation()}
            >
              <div className="flex items-center gap-2 text-base font-semibold">
                <CheckCircle2 size={18} className="text-[var(--color-safe)]" />
                已卸载，检测到残留，是否顺手清理？
              </div>
              <div className="mt-2 text-sm leading-relaxed text-[var(--color-text-secondary)]">
                「{residue.entry.name}」卸载后，电脑上还留着这些文件夹和设置项。勾选后即可清除，不需要的可以取消勾选。
              </div>

              <div className="mt-3 min-h-0 flex-1 space-y-3 overflow-y-auto pr-1">
                {residue.report.dirs.length > 0 && (
                  <div>
                    <div className="mb-1.5 text-xs font-semibold text-[var(--color-text-secondary)]">
                      残留文件夹
                    </div>
                    <ul className="space-y-1.5">
                      {residue.report.dirs.map((d) => {
                        const on = residue.checkedDirs.has(d.path);
                        return (
                          <li key={d.path}>
                            <label className="flex cursor-pointer items-start gap-2.5 rounded-xl bg-[var(--color-bg)] px-3.5 py-2.5">
                              <input
                                type="checkbox"
                                checked={on}
                                onChange={() =>
                                  setResidue((r) => {
                                    if (!r) return r;
                                    const s = new Set(r.checkedDirs);
                                    on ? s.delete(d.path) : s.add(d.path);
                                    return { ...r, checkedDirs: s };
                                  })
                                }
                                className="mt-0.5"
                              />
                              <span className="min-w-0 flex-1">
                                <span className="block break-all text-[13px] text-[var(--color-text-main)]">
                                  {d.path}
                                </span>
                                <span className="text-xs text-[var(--color-text-secondary)]">
                                  {formatBytes(d.size)}
                                </span>
                              </span>
                            </label>
                          </li>
                        );
                      })}
                    </ul>
                  </div>
                )}

                {residue.report.regKeys.length > 0 && (
                  <div>
                    <div className="mb-1.5 text-xs font-semibold text-[var(--color-text-secondary)]">
                      残留注册表项（当前用户）
                    </div>
                    <ul className="space-y-1.5">
                      {residue.report.regKeys.map((k) => {
                        const on = residue.checkedRegs.has(k);
                        return (
                          <li key={k}>
                            <label className="flex cursor-pointer items-start gap-2.5 rounded-xl bg-[var(--color-bg)] px-3.5 py-2.5">
                              <input
                                type="checkbox"
                                checked={on}
                                onChange={() =>
                                  setResidue((r) => {
                                    if (!r) return r;
                                    const s = new Set(r.checkedRegs);
                                    on ? s.delete(k) : s.add(k);
                                    return { ...r, checkedRegs: s };
                                  })
                                }
                                className="mt-0.5"
                              />
                              <span className="min-w-0 flex-1 break-all text-[13px] text-[var(--color-text-main)]">
                                {k}
                              </span>
                            </label>
                          </li>
                        );
                      })}
                    </ul>
                  </div>
                )}
              </div>

              <div className="mt-4 flex gap-3">
                <button
                  onClick={() => setResidue(null)}
                  className="flex-1 rounded-xl border border-[var(--color-line)] py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]"
                >
                  先留着
                </button>
                <button
                  disabled={
                    cleaningResidue || (residue.checkedDirs.size === 0 && residue.checkedRegs.size === 0)
                  }
                  onClick={doCleanResidue}
                  className="flex-1 rounded-xl bg-[var(--color-primary)] py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)] disabled:opacity-50"
                >
                  {cleaningResidue
                    ? "清理中…"
                    : `清理所选（${residue.checkedDirs.size + residue.checkedRegs.size}）`}
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* 卸载久未完成的顶部引导：避开居中弹出的卸载向导，固定在页面顶部让用户始终可见 */}
      <AnimatePresence>
        {uninHint && job?.status === "running" && (
          <motion.div
            initial={{ opacity: 0, y: -16 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -16 }}
            className="pointer-events-none absolute inset-x-0 top-4 z-[60] flex justify-center px-6"
          >
            <div className="pointer-events-auto flex max-w-xl items-start gap-2.5 rounded-xl border border-caution bg-warn-bg px-4 py-3 shadow-[var(--shadow-card-hover)]">
              <AlertTriangle size={18} className="mt-0.5 shrink-0 text-warn-text-strong" />
              <div className="text-sm leading-relaxed text-warn-text">
                这个软件弹出了自己的卸载向导窗口（可能挡在屏幕中间）。请在
                <b>那个窗口</b>上完成操作（点“下一步 / 卸载 / 是”），完成后这里会
                <b>自动继续</b>，不用管本窗口。
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* 结果/错误通知弹窗（问题一律弹窗，不在页面里内联平铺） */}
      <AnimatePresence>
        {notice && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="absolute inset-0 z-50 flex items-center justify-center bg-black/40 p-6"
            onClick={() => setNotice(null)}
          >
            <motion.div
              initial={{ scale: 0.94, y: 10 }}
              animate={{ scale: 1, y: 0 }}
              exit={{ scale: 0.94, y: 10 }}
              className="w-full max-w-md rounded-2xl bg-[var(--color-surface)] p-6 shadow-[var(--shadow-card-hover)]"
              onClick={(ev) => ev.stopPropagation()}
            >
              <div className="flex items-center gap-2 text-base font-semibold">
                {notice.kind === "error" ? (
                  <AlertTriangle size={18} className="text-[var(--color-caution)]" />
                ) : (
                  <CheckCircle2 size={18} className="text-[var(--color-safe)]" />
                )}
                {notice.kind === "error" ? "遇到一点问题" : "完成"}
              </div>
              <div className="mt-2 break-words text-sm leading-relaxed text-[var(--color-text-secondary)]">
                {notice.text}
              </div>
              <div className="mt-5 flex justify-end">
                <button
                  onClick={() => setNotice(null)}
                  className="rounded-xl bg-[var(--color-primary)] px-6 py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                >
                  知道了
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
