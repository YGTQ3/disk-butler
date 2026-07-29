import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { motion, AnimatePresence } from "framer-motion";
import {
  HeartHandshake,
  Loader2,
  CheckCircle2,
  XCircle,
  FolderOpen,
  ShieldCheck,
  Mail,
} from "lucide-react";
import { CollectResult, openInExplorer } from "../types";

type Phase = "intro" | "running" | "done" | "error";

/** 接收贡献报告的邮箱（留空 = 隐藏“用邮箱发给我们”按钮） */
const FEEDBACK_EMAIL = "ygtq1021@126.com";

/** 拉起用户自己的邮件客户端写草稿（mailto 协议，软件本身不联网不发送） */
async function draftEmail(jsonPath: string) {
  const fileName = jsonPath.split("\\").pop() ?? jsonPath;
  const subject = encodeURIComponent("C盘管家·规则采集报告");
  const body = encodeURIComponent(
    `你好，这是我的规则采集报告。\n\n` +
      `请把桌面上的 ${fileName} 拖进这封邮件作为附件。\n\n` +
      `这台电脑主要用来：（办公 / 游戏 / 剪辑 / 家用）\n`
  );
  try {
    await openUrl(`mailto:${FEEDBACK_EMAIL}?subject=${subject}&body=${body}`);
  } catch (e) {
    alert(`没能打开邮件客户端：${String(e)}\n\n你也可以手动发邮件到：${FEEDBACK_EMAIL}`);
  }
}

/** 「帮它认识更多软件」：本地生成规则采集报告，用户自主决定是否分享 */
export default function ContributeModal({ onClose }: { onClose: () => void }) {
  const [phase, setPhase] = useState<Phase>("intro");
  const [fullMode, setFullMode] = useState(false);
  const [result, setResult] = useState<CollectResult | null>(null);
  const [error, setError] = useState("");

  async function start() {
    setPhase("running");
    try {
      const r = await invoke<CollectResult>("collect_rules", {
        includeDrives: fullMode,
      });
      setResult(r);
      setPhase("done");
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  }

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-6"
        onClick={() => phase !== "running" && onClose()}
      >
        <motion.div
          initial={{ scale: 0.94, y: 10 }}
          animate={{ scale: 1, y: 0 }}
          exit={{ scale: 0.94, y: 10 }}
          className="w-full max-w-[40rem] rounded-2xl bg-[var(--color-surface)] p-7 shadow-[var(--shadow-card-hover)]"
          onClick={(e) => e.stopPropagation()}
        >
          {phase === "intro" && (
            <>
              <div className="flex items-center gap-3">
                <span className="flex h-11 w-11 items-center justify-center rounded-xl bg-[var(--color-primary-soft)] text-[var(--color-primary-dark)]">
                  <HeartHandshake size={24} />
                </span>
                <div className="text-lg font-semibold">帮它认识更多软件</div>
              </div>
              <div className="mt-3 text-[15px] leading-relaxed text-[var(--color-text-secondary)]">
                每台电脑装的软件不一样。这个功能会扫描你机器上的
                <b className="text-[var(--color-text-main)]">缓存分布线索</b>
                ，在<b className="text-[var(--color-text-main)]">桌面</b>
                生成一份报告——把它发给我们，你电脑上那些小众软件的缓存就能被下个版本安全地识别。
              </div>

              {/* 三不承诺 + 隐私边界 */}
              <div className="mt-4 rounded-xl bg-[var(--color-bg)] p-3.5 text-sm leading-relaxed text-[var(--color-text-secondary)]">
                <div className="mb-1.5 flex items-center gap-1.5 font-medium text-[var(--color-primary-dark)]">
                  <ShieldCheck size={16} />
                  先说清楚会发生什么
                </div>
                · 不删除、不修改任何东西，也不联网——只在桌面写两个报告文件；
                <br />
                · 报告里只有软件名、目录名和大小，
                <b>没有文件名、没有文件内容、没有你的用户名</b>；
                <br />
                · 报告生成后<b>只存在你的电脑上</b>，发不发、发给谁，完全由你决定。
              </div>

              {/* 模式选择 */}
              <div className="mt-4 space-y-2">
                <button
                  onClick={() => setFullMode(false)}
                  className={[
                    "w-full rounded-xl border px-4 py-3 text-left text-[15px] transition-colors",
                    !fullMode
                      ? "border-[var(--color-primary)] bg-[var(--color-primary-soft)]"
                      : "border-[var(--color-line)] hover:bg-[var(--color-bg)]",
                  ].join(" ")}
                >
                  <div className="font-medium">基础模式（推荐）</div>
                  <div className="mt-0.5 text-[13px] text-[var(--color-text-secondary)]">
                    只扫描软件缓存区（AppData 等），约 1 分钟
                  </div>
                </button>
                <button
                  onClick={() => setFullMode(true)}
                  className={[
                    "w-full rounded-xl border px-4 py-3 text-left text-[15px] transition-colors",
                    fullMode
                      ? "border-[var(--color-primary)] bg-[var(--color-primary-soft)]"
                      : "border-[var(--color-line)] hover:bg-[var(--color-bg)]",
                  ].join(" ")}
                >
                  <div className="font-medium">完整模式</div>
                  <div className="mt-0.5 text-[13px] text-[var(--color-text-secondary)]">
                    另外收集各磁盘的大目录线索（磁盘部分已用内置极速引擎，大幅提速），通常几十秒。
                    ⚠ 报告会包含 D 盘等根目录下大文件夹的名字，发送前请自行审查
                  </div>
                </button>
              </div>

              <div className="mt-5 flex justify-end gap-2.5">
                <button
                  onClick={onClose}
                  className="rounded-xl px-5 py-2.5 text-[15px] text-[var(--color-text-secondary)] hover:bg-[var(--color-bg)]"
                >
                  先不了
                </button>
                <button
                  onClick={() => void start()}
                  className="rounded-xl bg-[var(--color-primary)] px-6 py-2.5 text-[15px] font-medium text-white hover:bg-[var(--color-primary-dark)]"
                >
                  开始生成报告
                </button>
              </div>
            </>
          )}

          {phase === "running" && (
            <div className="flex flex-col items-center py-10">
              <Loader2 size={40} className="animate-spin text-[var(--color-primary)]" />
              <div className="mt-5 text-[15px] font-medium">
                正在扫描{fullMode ? "（完整模式，通常几十秒）" : "（约 1 分钟内）"}……
              </div>
              <div className="mt-2 text-sm text-[var(--color-text-secondary)]">
                只读取目录名和大小，不碰任何文件内容，期间可正常使用电脑
              </div>
            </div>
          )}

          {phase === "done" && result && (
            <>
              <div className="flex flex-col items-center pt-3">
                <motion.div
                  initial={{ scale: 0 }}
                  animate={{ scale: 1 }}
                  transition={{ type: "spring", stiffness: 260, damping: 16 }}
                >
                  <CheckCircle2 size={52} className="text-[var(--color-primary)]" />
                </motion.div>
                <div className="mt-3.5 text-lg font-semibold">报告已生成到桌面</div>
                <div className="mt-2 text-center text-sm leading-relaxed text-[var(--color-text-secondary)]">
                  收录 {result.softwareCount} 个软件、{result.dirRows} 个目录线索
                  {result.driveRows > 0 && `、${result.driveRows} 条磁盘大目录`}
                  ，用时 {result.elapsedSecs < 60 ? `${Math.max(1, result.elapsedSecs)} 秒` : `${Math.round(result.elapsedSecs / 60)} 分钟`}
                </div>
              </div>
              <div className="mt-4 space-y-2.5 rounded-xl bg-[#FFFBEB] p-4 text-sm leading-relaxed text-[#92400E]">
                <div className="flex items-start gap-2.5">
                  <span className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-[#FDE9CE] text-xs font-bold text-[#B45309]">1</span>
                  <div>可以先打开报告看一遍——分不分享，你说了算。</div>
                </div>
                <div className="flex items-start gap-2.5">
                  <span className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-[#FDE9CE] text-xs font-bold text-[#B45309]">2</span>
                  <div>发送时，把桌面上的 <b>.json 文件</b>拖进邮件附件。</div>
                </div>
              </div>
              <div className="mt-4 flex justify-end gap-2.5">
                <button
                  onClick={onClose}
                  className="rounded-xl px-5 py-2.5 text-[15px] text-[var(--color-text-secondary)] hover:bg-[var(--color-bg)]"
                >
                  关闭
                </button>
                <button
                  onClick={() => void openInExplorer(result.jsonPath, false)}
                  className={[
                    "flex items-center gap-1.5 rounded-xl px-5 py-2.5 text-[15px] font-medium",
                    FEEDBACK_EMAIL
                      ? "border border-[var(--color-primary)] text-[var(--color-primary-dark)] hover:bg-[var(--color-primary-soft)]"
                      : "bg-[var(--color-primary)] text-white hover:bg-[var(--color-primary-dark)]",
                  ].join(" ")}
                >
                  <FolderOpen size={16} />
                  查看报告位置
                </button>
                {FEEDBACK_EMAIL && (
                  <button
                    onClick={() => void draftEmail(result.jsonPath)}
                    className="flex items-center gap-1.5 rounded-xl bg-[var(--color-primary)] px-6 py-2.5 text-[15px] font-medium text-white hover:bg-[var(--color-primary-dark)]"
                  >
                    <Mail size={16} />
                    用邮箱发给我们
                  </button>
                )}
              </div>
            </>
          )}

          {phase === "error" && (
            <>
              <div className="flex flex-col items-center pt-2">
                <XCircle size={40} className="text-[#DC2626]" />
                <div className="mt-3 text-base font-semibold">没能生成报告</div>
                <div className="mt-2 max-h-32 overflow-y-auto rounded-lg bg-[var(--color-bg)] px-3 py-2 text-xs text-[var(--color-text-secondary)]">
                  {error}
                </div>
              </div>
              <div className="mt-4 flex justify-end">
                <button
                  onClick={onClose}
                  className="rounded-xl bg-[var(--color-primary)] px-5 py-2 text-sm font-medium text-white hover:bg-[var(--color-primary-dark)]"
                >
                  知道了
                </button>
              </div>
            </>
          )}
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
