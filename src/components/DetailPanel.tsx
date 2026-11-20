import { Folder, File, Info, FolderOpen, FolderInput } from "lucide-react";
import {
  TreeNode,
  CATEGORY_COLOR,
  CATEGORY_LABEL,
  SAFETY_META,
  formatBytes,
  openInExplorer,
} from "../types";

/** 转移功能的下限：小于 100MB 的目录在资源管理器里直接挪就行，不值得走“复制→校验→建链”流水线 */
const TRANSFER_MIN_SIZE = 100 * 1024 * 1024;

interface Props {
  node: TreeNode | null;
  /** 根大小，用于计算占比 */
  rootSize: number;
  /** 发起「转移到其他磁盘」（仅符合条件的目录出现入口） */
  onTransfer?: (node: TreeNode) => void;
}

/** 转移入口候选：目录、非聚合伪节点、够大（是否可转移还取决于安全等级，由按钮态区分） */
function transferCandidate(node: TreeNode): boolean {
  return (
    node.isDir &&
    !node.path.endsWith("::__others__") &&
    node.size >= TRANSFER_MIN_SIZE
  );
}

export default function DetailPanel({ node, rootSize, onTransfer }: Props) {
  if (!node) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
        <Info size={28} className="text-[var(--color-line)]" />
        <p className="text-sm leading-relaxed text-[var(--color-text-secondary)]">
          把鼠标移到左边的方块上
          <br />
          点击查看它的详细说明
        </p>
      </div>
    );
  }

  const safety = SAFETY_META[node.safety];
  const percent = rootSize > 0 ? ((node.size / rootSize) * 100).toFixed(1) : "0";

  return (
    <div className="flex h-full flex-col gap-4 p-5">
      {/* 名称与分类 */}
      <div>
        <div className="flex items-center gap-2">
          {node.isDir ? (
            <Folder size={18} style={{ color: CATEGORY_COLOR[node.category] }} />
          ) : (
            <File size={18} style={{ color: CATEGORY_COLOR[node.category] }} />
          )}
          <span className="truncate text-base font-semibold" title={node.name}>
            {node.name}
          </span>
        </div>
        <div className="mt-1 text-sm text-[var(--color-text-secondary)]">{node.friendlyName}</div>
      </div>

      {/* 徽章行 */}
      <div className="flex flex-wrap items-center gap-2">
        <span
          className="rounded-full px-2.5 py-1 text-xs font-medium text-white"
          style={{ background: CATEGORY_COLOR[node.category] }}
        >
          {CATEGORY_LABEL[node.category]}
        </span>
        <span
          className="rounded-full px-2.5 py-1 text-xs font-medium text-white"
          style={{ background: safety.color }}
        >
          {safety.label}
        </span>
      </div>

      {/* 大小 */}
      <div className="rounded-xl bg-[var(--color-bg)] p-4">
        <div className="text-2xl font-bold">{formatBytes(node.size)}</div>
        <div className="mt-0.5 text-xs text-[var(--color-text-secondary)]">
          占当前视图的 {percent}%
        </div>
      </div>

      {/* 说明：这是什么、能不能动 */}
      <div>
        <div className="mb-1.5 text-xs font-medium text-[var(--color-text-secondary)]">
          这是什么？能不能动？
        </div>
        <p className="text-sm leading-relaxed">{node.description}</p>
      </div>

      {/* 路径 */}
      <div className="mt-auto">
        <div className="mb-1 text-xs font-medium text-[var(--color-text-secondary)]">完整路径</div>
        <div
          className="select-text break-all rounded-lg bg-[var(--color-bg)] px-3 py-2 text-xs text-[var(--color-text-secondary)]"
          title={node.path}
        >
          {node.path.replace(/::__others__$/, "（合并的小项目）")}
        </div>
        {!node.path.endsWith("::__others__") && (
          <>
            <button
              onClick={() => void openInExplorer(node.path, node.isDir)}
              className="mt-2 flex w-full items-center justify-center gap-1.5 rounded-xl border border-[var(--color-line)] bg-[var(--color-surface)] px-3 py-2 text-xs font-medium text-[var(--color-primary-dark)] transition-colors hover:bg-[var(--color-bg)]"
            >
              <FolderOpen size={14} />
              在文件夹中打开
            </button>
            {transferCandidate(node) && onTransfer && (
              <>
                <button
                  onClick={() => onTransfer(node)}
                  className="mt-2 flex w-full items-center justify-center gap-1.5 rounded-xl bg-[var(--color-primary)] px-3 py-2 text-xs font-medium text-white transition-colors hover:bg-[var(--color-primary-dark)]"
                  title="把整个目录搬到另一块磁盘，旧路径自动兼容，为当前盘腾出空间"
                >
                  <FolderInput size={14} />
                  转移到其他磁盘
                </button>
                {node.safety === "keep" && (
                  <p className="mt-1.5 px-1 text-[11px] leading-relaxed text-[var(--color-text-secondary)]">
                    该目录被知识库标记为「请勿删除」。转移不删数据（旧路径仍可用），
                    若它是系统目录，预检会拒绝并说明原因。
                  </p>
                )}
              </>
            )}
          </>
        )}
      </div>
    </div>
  );
}
