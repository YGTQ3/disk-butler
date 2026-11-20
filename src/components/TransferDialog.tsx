import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { motion } from "framer-motion";
import {
  AlertTriangle,
  CheckCircle2,
  FolderInput,
  FolderOutput,
  HardDrive,
  Loader2,
  X,
} from "lucide-react";
import {
  DriveInfo,
  TreeNode,
  TransferPlan,
  TransferProgress,
  TransferReport,
  formatBytes,
  formatCount,
} from "../types";

/**
 * 透视页「转移到其他磁盘」弹窗。
 * 状态机沿用 docs/ui-reference 范式：intro（引导+选目标）→ analyzing（只读预检）
 * → plan（结果面板）→ confirm（红色确认）→ running（进度）→ done（完成），
 * error 为各阶段共用终点（大白话原因 + 返回重选）。
 */
type TPhase = "intro" | "analyzing" | "plan" | "confirm" | "running" | "done" | "error";

interface Props {
  node: TreeNode;
  /** 盘符列表（用于「其他磁盘」快捷目标） */
  drives: DriveInfo[];
  onClose: () => void;
  /** 转移成功收尾：通知页面重扫源盘、刷新水位条 */
  onTransferred: (sourceVolume: string) => void;
}

/** 阶段文案（running 进度条上方） */
const PHASE_LABEL: Record<string, string> = {
  measure: "正在统计目录内容",
  copy: "正在复制到目标磁盘",
  verify: "正在核对复制结果",
  swap: "正在做转移前的原子切换",
  link: "正在建立路径兼容链接",
  cleanup: "正在删除源盘临时副本",
};

export default function TransferDialog({ node, drives, onClose, onTransferred }: Props) {
  const [phase, setPhase] = useState<TPhase>("intro");
  const [targetDir, setTargetDir] = useState("");
  const [plan, setPlan] = useState<TransferPlan | null>(null);
  const [errorMsg, setErrorMsg] = useState("");
  /** 取消后回到选目标界面的提示条 */
  const [cancelledNote, setCancelledNote] = useState("");
  const [progress, setProgress] = useState<TransferProgress | null>(null);
  const [report, setReport] = useState<TransferReport | null>(null);
  /** running 阶段点「收起」时置位，避免完成事件再切弹窗 */
  const detachedRef = useRef(false);

  /** 源盘挂载点（与页面 selected/scannedDrive 同口径：如 "C:\"；盘符字母兜底） */
  const sourceLetter = node.path.slice(0, 1).toUpperCase();
  const sourceVolume =
    drives.find((d) => d.letter.toUpperCase() === sourceLetter)?.mountPoint ?? `${sourceLetter}\\`;
  /** 其他内置磁盘快捷目标（排除源盘） */
  const otherDrives = drives.filter((d) => d.letter.toUpperCase() !== sourceLetter);
  const canSubmit = targetDir.trim().length > 0;

  // 监听后端进度事件（弹窗挂载期间）
  useEffect(() => {
    const un = listen<TransferProgress>("transfer-progress", (e) => {
      if (detachedRef.current) return; // 已收起后台跑：不再打扰
      const p = e.payload;
      setProgress(p);
      if (p.done) {
        if (p.phase === "done" && p.report) {
          setReport(p.report);
          setPhase("done");
        } else if (p.phase === "error") {
          setErrorMsg(p.message);
          setPhase("error");
        } else if (p.phase === "cancelled") {
          setCancelledNote(p.message);
          setPhase("intro");
        }
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  /** 系统原生目录选择 */
  async function pickTarget() {
    const picked = await openDialog({ directory: true, multiple: false, title: "选择要转移到的目录" });
    if (typeof picked === "string" && picked) setTargetDir(picked);
  }

  /** 只读预检：合法性 + 统计规模（不改任何文件） */
  async function runPreflight() {
    if (!canSubmit) return;
    setCancelledNote("");
    setErrorMsg("");
    setPhase("analyzing");
    try {
      const p = await invoke<TransferPlan>("transfer_preflight", { source: node.path, target: targetDir });
      setPlan(p);
      setPhase("plan");
    } catch (e) {
      setErrorMsg(String(e));
      setPhase("error");
    }
  }

  /** 启动流水线（立即返回；进度经事件推送） */
  async function startTransfer() {
    if (!plan) return;
    setProgress(null);
    setPhase("running");
    try {
      await invoke("start_transfer", { source: plan.sourcePath, target: targetDir });
    } catch (e) {
      setErrorMsg(String(e));
      setPhase("error");
    }
  }

  function close() {
    detachedRef.current = true;
    onClose();
  }

  function done() {
    onTransferred(sourceVolume);
    close();
  }

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="absolute inset-0 z-30 flex items-center justify-center bg-black/30 p-6"
      onClick={close}
    >
      <motion.div
        initial={{ scale: 0.94, y: 10 }}
        animate={{ scale: 1, y: 0 }}
        exit={{ scale: 0.94, y: 10 }}
        className="relative w-full max-w-lg rounded-2xl bg-[var(--color-surface)] p-7 shadow-[var(--shadow-card-hover)]"
        onClick={(e) => e.stopPropagation()}
      >
        {/* 右上角关闭（running 阶段隐藏，防误触中断流程认知） */}
        {phase !== "running" && (
          <button
            onClick={close}
            className="absolute right-4 top-4 rounded-lg p-1.5 text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg)]"
            title="关闭"
          >
            <X size={16} />
          </button>
        )}

        {/* ---------- 引导 + 选目标 ---------- */}
        {phase === "intro" && (
          <>
            <div className="text-lg font-semibold">
              把「{node.name}」转移到其他磁盘
            </div>
            <div className="mt-1 text-xs text-[var(--color-text-secondary)]">
              共约 {formatBytes(node.size)} · 适合放不下的目录整体搬家
            </div>

            {cancelledNote && (
              <div className="mt-3 rounded-xl bg-[var(--color-warn-bg)] px-4 py-2.5 text-xs text-[var(--color-warn-text)]">
                {cancelledNote}
              </div>
            )}

            {/* 第一步：选目标目录 */}
            <div className="mt-4 text-sm font-medium">转移到哪里？</div>
            <div className="mt-2 flex gap-2">
              <button
                onClick={() => void pickTarget()}
                className="flex min-w-0 flex-1 items-center gap-2 rounded-xl border border-[var(--color-line)] bg-[var(--color-bg)] px-3 py-2.5 text-left text-sm transition-colors hover:border-[var(--color-primary)]"
              >
                <FolderOutput size={16} className="shrink-0 text-[var(--color-primary)]" />
                <span className={["truncate", targetDir ? "" : "text-[var(--color-text-secondary)]"].join(" ")}>
                  {targetDir || "选择目标目录…"}
                </span>
              </button>
            </div>
            {otherDrives.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-2">
                {otherDrives.map((d) => (
                  <button
                    key={d.mountPoint}
                    onClick={() => setTargetDir(d.mountPoint)}
                    className={[
                      "flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-xs transition-colors",
                      targetDir === d.mountPoint
                        ? "border-[var(--color-primary)] bg-[var(--color-primary-soft)] text-[var(--color-primary-dark)]"
                        : "border-[var(--color-line)] text-[var(--color-text-secondary)] hover:border-[var(--color-primary)]",
                    ].join(" ")}
                  >
                    <HardDrive size={12} />
                    {d.letter} 盘 · 剩 {formatBytes(d.free)}
                  </button>
                ))}
              </div>
            )}

            {/* 开始前须知 */}
            <div className="mt-5 text-sm font-medium">开始前，先看这 3 件事</div>
            <div className="mt-2 space-y-2">
              {[
                ["1", "数据先完整复制到目标磁盘，逐文件核对无误后才动源目录。"],
                ["2", "转移完成后原位置会留一个「传送门」（Junction 链接）：软件仍按老路径找数据，自动指向新位置，全程免管理员权限。"],
                ["3", `转移期间请勿使用该目录里的文件、不要关机；目标磁盘剩余空间需大于 ${formatBytes(node.size)}。`],
              ].map(([n, text]) => (
                <div key={n} className="flex items-center gap-3.5 rounded-xl bg-[var(--color-bg)] px-4 py-3">
                  <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-primary)] text-sm font-bold text-white">
                    {n}
                  </span>
                  <span className="min-w-0 text-sm leading-relaxed">{text}</span>
                </div>
              ))}
            </div>

            {/* 绿色安全条 */}
            <div className="mt-4 rounded-xl bg-[var(--color-primary-soft)] px-4 py-2.5 text-xs text-[var(--color-primary-dark)]">
              全程可放心：复制阶段取消或失败，源目录原样保留，不会丢任何数据。
            </div>
            {node.safety === "keep" && (
              <div className="mt-2 flex items-start gap-2 rounded-xl bg-[var(--color-warn-bg)] px-4 py-2.5 text-xs text-[var(--color-warn-text)]">
                <AlertTriangle size={13} className="mt-0.5 shrink-0 text-[var(--color-warn-icon)]" />
                该目录被知识库标记为「请勿删除」。转移只是搬数据（旧路径仍可用），不会删除；如果它确实是系统目录，下一步预检会拒绝并说明原因。
              </div>
            )}

            <div className="mt-5 flex gap-3">
              <button
                onClick={close}
                className="flex-1 rounded-xl border border-[var(--color-line)] py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]"
              >
                取消
              </button>
              <button
                onClick={() => void runPreflight()}
                disabled={!canSubmit}
                className="flex flex-[1.4] items-center justify-center gap-2 rounded-xl bg-[var(--color-primary)] py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)] disabled:opacity-40"
              >
                <FolderInput size={16} />
                校验并查看方案
              </button>
            </div>
          </>
        )}

        {/* ---------- 只读预检中 ---------- */}
        {phase === "analyzing" && (
          <div className="flex flex-col items-center gap-4 py-10 text-center">
            <Loader2 size={36} className="animate-spin text-[var(--color-primary)]" />
            <div className="text-sm font-medium">正在检查目录与目标磁盘…</div>
            <div className="text-xs text-[var(--color-text-secondary)]">
              只读取信息做预检，不复制不删除，大目录约需几秒到几十秒
            </div>
          </div>
        )}

        {/* ---------- 结果面板（左明细 + 右绿色底面板） ---------- */}
        {phase === "plan" && plan && (
          <>
            <div className="text-lg font-semibold">转移方案</div>
            <div className="mt-1 text-xs text-[var(--color-text-secondary)]">
              这是只读预检的结果，还没有动任何文件
            </div>

            <div className="mt-4 flex gap-3">
              {/* 左：明细 */}
              <div className="flex-1 space-y-2.5 rounded-xl bg-[var(--color-bg)] p-3.5">
                <Row label="源目录" value={plan.sourceName} small={plan.sourcePath} />
                <Row label="总大小" value={formatBytes(plan.sourceSize)} strong />
                <Row label="文件" value={`${formatCount(plan.fileCount)} 个`} />
                <Row label="将转移到" value={plan.destination} />
                <Row label={`${plan.targetLetter} 盘剩余`} value={formatBytes(plan.targetFree)} strong />
              </div>
              {/* 右：绿色底面板 */}
              <div className="w-48 shrink-0 rounded-xl bg-[var(--color-primary-soft)] p-4 text-center">
                <div className="text-xs text-[var(--color-primary-dark)]">预计为源盘腾出</div>
                <div className="mt-1 text-3xl font-bold text-[var(--color-primary-dark)]">
                  {formatBytes(plan.sourceSize)}
                </div>
                <button
                  onClick={() => setPhase("confirm")}
                  className="mt-3 w-full rounded-xl bg-[var(--color-primary)] py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                >
                  开始转移
                </button>
                <button
                  onClick={() => setPhase("intro")}
                  className="mt-2 w-full text-xs text-[var(--color-primary-dark)] underline-offset-2 hover:underline"
                >
                  重选目标目录
                </button>
              </div>
            </div>

            {plan.warnings.map((w, i) => (
              <div
                key={i}
                className="mt-2 flex items-start gap-2 rounded-xl bg-[var(--color-warn-bg)] px-4 py-2.5 text-xs text-[var(--color-warn-text)]"
              >
                <AlertTriangle size={13} className="mt-0.5 shrink-0 text-[var(--color-warn-icon)]" />
                {w}
              </div>
            ))}
          </>
        )}

        {/* ---------- 确认弹窗要点 ---------- */}
        {phase === "confirm" && plan && (
          <>
            <div className="text-lg font-semibold">转移前，最后核对一遍</div>
            <div className="mt-4 space-y-2">
              <Step n="1" text={`先复制 ${formatBytes(plan.sourceSize)} 到 ${plan.targetLetter} 盘（${plan.destination}），复制完成逐文件核对。`} />
              <Step n="2" text="核对无误后原目录改名保留，原位置立即建立链接指向新位置——软件照常工作。" />
              <div className="flex items-center gap-3.5 rounded-xl border border-[var(--color-danger-border)] bg-[var(--color-danger-bg-soft)] px-4 py-3">
                <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-alert)] text-sm font-bold text-white">
                  3
                </span>
                <span className="min-w-0 text-sm leading-relaxed text-[var(--color-danger-dark)]">
                  最后一步会删除源盘上的临时副本（链接生效后才删）。转移期间请勿使用该目录、不要关机；个别文件若被软件占用会跳过并如实报告，不影响链接生效。
                </span>
              </div>
            </div>
            <div className="mt-5 flex gap-3">
              <button
                onClick={() => setPhase("plan")}
                className="flex-1 rounded-xl border border-[var(--color-line)] py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]"
              >
                再想想
              </button>
              <button
                onClick={() => void startTransfer()}
                className="flex-[1.4] rounded-xl bg-[var(--color-primary)] py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)]"
              >
                确认转移
              </button>
            </div>
          </>
        )}

        {/* ---------- 进行中 ---------- */}
        {phase === "running" && (
          <div className="flex flex-col items-center gap-5 py-4">
            <div className="flex items-center gap-2 text-base font-medium">
              <Loader2 size={18} className="animate-spin text-[var(--color-primary)]" />
              {PHASE_LABEL[progress?.phase ?? "copy"] ?? "正在转移"}
            </div>
            {/* 进度条 */}
            <div className="w-full">
              <div className="h-2 w-full overflow-hidden rounded-full bg-[var(--color-primary-soft)]">
                <div
                  className="h-full rounded-full bg-[var(--color-primary)] transition-[width] duration-300 ease-out"
                  style={{ width: `${progress ? Math.round(progress.percent) : 0}%` }}
                />
              </div>
              <div className="mt-2 flex justify-between text-xs text-[var(--color-text-secondary)]">
                <span>{progress?.message || "正在准备…"}</span>
                <span>{progress ? `${Math.round(progress.percent)}%` : "0%"}</span>
              </div>
            </div>
            <div className="w-full truncate rounded-lg bg-[var(--color-bg)] px-3 py-2 text-[11px] text-[var(--color-text-secondary)]" title={progress?.currentPath}>
              {progress?.currentPath || node.path}
            </div>
            <div className="flex w-full gap-3">
              <button
                onClick={() => void invoke("cancel_transfer")}
                disabled={progress?.phase !== "copy"}
                className="flex-1 rounded-xl border border-[var(--color-line)] py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)] disabled:opacity-40"
              >
                取消转移
              </button>
              <button
                onClick={() => {
                  detachedRef.current = true;
                  onClose();
                }}
                className="flex-1 rounded-xl border border-[var(--color-line)] py-2.5 text-sm font-medium text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg)]"
                title="弹窗关闭，转移在后台继续；完成后可在透视页重新扫描查看"
              >
                收起（后台继续）
              </button>
            </div>
            <div className="text-[11px] text-[var(--color-text-secondary)]">
              复制阶段取消/失败都会自动清理半成品，源目录原样保留
            </div>
          </div>
        )}

        {/* ---------- 完成 ---------- */}
        {phase === "done" && report && (
          <div className="flex flex-col items-center gap-4 py-2 text-center">
            <CheckCircle2 size={44} className="text-[var(--color-safe)]" />
            <div className="text-base font-medium">转移完成！</div>
            <div className="text-xs text-[var(--color-text-secondary)]">本次为 {sourceVolume} 盘释放</div>
            <div className="text-4xl font-bold text-[var(--color-primary-dark)]">
              {formatBytes(report.freedBytes)}
            </div>
            <div className="w-full space-y-1.5 rounded-xl bg-[var(--color-bg)] p-3.5 text-left text-xs">
              <div className="flex justify-between">
                <span className="text-[var(--color-text-secondary)]">新位置</span>
                <b className="text-[var(--color-text-main)]">{plan?.destination ?? ""}</b>
              </div>
              <div className="flex justify-between">
                <span className="text-[var(--color-text-secondary)]">耗时</span>
                <b className="text-[var(--color-text-main)]">{report.durationSecs} 秒</b>
              </div>
              <div className="flex justify-between">
                <span className="text-[var(--color-text-secondary)]">原位置</span>
                <b className="text-[var(--color-text-main)]">链接已生效（{node.path}）</b>
              </div>
            </div>
            {report.skipped.length > 0 && (
              <div className="w-full rounded-xl bg-[var(--color-warn-bg)] px-4 py-2.5 text-left text-xs text-[var(--color-warn-text)]">
                有 {report.skipped.length} 个链接子项被跳过（保留在原地）
              </div>
            )}
            {report.tempLeftover > 0 && (
              <div className="w-full rounded-xl bg-[var(--color-warn-bg)] px-4 py-2.5 text-left text-xs text-[var(--color-warn-text)]">
                临时副本有 {formatBytes(report.tempLeftover)} 因文件被占用未删净，不影响使用；可稍后在资源管理器手动删除「{report.tempLeftoverPath ?? node.path}」
              </div>
            )}
            <button
              onClick={done}
              className="mt-1 w-full rounded-xl bg-[var(--color-primary)] py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)]"
            >
              好的，重新扫描看看
            </button>
          </div>
        )}

        {/* ---------- 出错 ---------- */}
        {phase === "error" && (
          <div className="flex flex-col items-center gap-4 py-4 text-center">
            <AlertTriangle size={36} className="text-[var(--color-warn-icon)]" />
            <div className="text-base font-medium">没能完成转移</div>
            <div className="max-h-40 w-full overflow-y-auto whitespace-pre-wrap rounded-xl bg-[var(--color-bg)] px-4 py-3 text-left text-xs leading-relaxed text-[var(--color-text-secondary)]">
              {errorMsg}
            </div>
            <div className="flex w-full gap-3">
              <button
                onClick={() => setPhase("intro")}
                className="flex-1 rounded-xl border border-[var(--color-line)] py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-bg)]"
              >
                返回重选
              </button>
              <button
                onClick={close}
                className="flex-1 rounded-xl bg-[var(--color-primary)] py-2.5 text-sm font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)]"
              >
                关闭
              </button>
            </div>
          </div>
        )}
      </motion.div>
    </motion.div>
  );
}

/** 明细行：标签 + 值（可选第二行小字路径） */
function Row({ label, value, strong, small }: { label: string; value: string; strong?: boolean; small?: string }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="shrink-0 text-sm text-[var(--color-text-secondary)]">{label}</span>
      <span className="min-w-0 text-right">
        {/* ui-reference：数字/重点值用更黑更粗，普通值次之 */}
        <b className={["text-sm", strong ? "font-bold text-[var(--color-text-main)]" : "text-[var(--color-text-secondary)]"].join(" ")}>
          {value}
        </b>
        {small && (
          <span className="block max-w-[240px] truncate text-[10px] text-[var(--color-text-secondary)]" title={small}>
            {small}
          </span>
        )}
      </span>
    </div>
  );
}

/** 确认弹窗编号要点 */
function Step({ n, text }: { n: string; text: string }) {
  return (
    <div className="flex items-center gap-3.5 rounded-xl bg-[var(--color-bg)] px-4 py-3">
      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--color-primary)] text-sm font-bold text-white">
        {n}
      </span>
      <span className="min-w-0 text-sm leading-relaxed">{text}</span>
    </div>
  );
}
