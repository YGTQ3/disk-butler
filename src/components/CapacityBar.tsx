import { TreeNode, CATEGORY_COLOR, CATEGORY_LABEL, Category, formatBytes } from "../types";

interface Props {
  /** 根节点（用于计算各分类占比） */
  root: TreeNode | null;
  total: number;
  free: number;
}

const CATEGORY_ORDER: Category[] = [
  "system",
  "software",
  "cache",
  "personal",
  "systemFile",
  "other",
];

/** 按分类聚合根节点下各直接子项（递归到叶子太慢，这里用一层子项的分类近似） */
function aggregateByCategory(root: TreeNode): Map<Category, number> {
  const map = new Map<Category, number>();
  const walk = (node: TreeNode, depth: number) => {
    // 只递归两层做分类聚合，够用且快
    if (depth >= 2 || !node.children || node.children.length === 0) {
      map.set(node.category, (map.get(node.category) ?? 0) + node.size);
      return;
    }
    for (const c of node.children) walk(c, depth + 1);
  };
  if (root.children) {
    for (const c of root.children) walk(c, 1);
  } else {
    map.set(root.category, root.size);
  }
  return map;
}

export default function CapacityBar({ root, total, free }: Props) {
  const used = total - free;
  const segments = root ? aggregateByCategory(root) : new Map<Category, number>();
  const sorted = CATEGORY_ORDER.map((cat) => ({
    cat,
    size: segments.get(cat) ?? 0,
  })).filter((s) => s.size > 0);

  return (
    <div className="w-full">
      <div className="mb-2 flex items-baseline justify-between text-sm">
        <span className="text-[var(--color-text-secondary)]">
          已用 <span className="font-semibold text-[var(--color-text-main)]">{formatBytes(used)}</span>
          {" / "}
          {formatBytes(total)}
        </span>
        <span className="text-[var(--color-text-secondary)]">
          剩余 <span className="font-semibold text-[var(--color-primary-dark)]">{formatBytes(free)}</span>
        </span>
      </div>

      {/* 分段水位条：已用按分类着色，剩余空间用空状态样式（镜面底色 + 描边）表示“还空着” */}
      <div className="flex h-3 w-full overflow-hidden rounded-full border border-[var(--color-line)] bg-[var(--color-bg)]">
        {root ? (
          sorted.map((s) => (
            <div
              key={s.cat}
              style={{
                width: `${(s.size / total) * 100}%`,
                background: CATEGORY_COLOR[s.cat],
              }}
              title={`${CATEGORY_LABEL[s.cat]}：${formatBytes(s.size)}`}
            />
          ))
        ) : (
          <div
            style={{ width: `${(used / total) * 100}%` }}
            className="bg-[var(--color-cat-other)]"
            title={`已用：${formatBytes(used)}`}
          />
        )}
        {/* 剩余段：保持空状态底色不填充，悬停可看具体剩余量；flex-1 吸收百分比舍入尾差 */}
        <div className="min-w-0 flex-1" title={`剩余空间：${formatBytes(free)}`} />
      </div>

      {/* 图例 */}
      {root && (
        <div className="mt-3 flex flex-wrap gap-x-4 gap-y-1.5">
          {sorted.map((s) => (
            <div key={s.cat} className="flex items-center gap-1.5 text-xs text-[var(--color-text-secondary)]">
              <span
                className="inline-block h-2.5 w-2.5 rounded-sm"
                style={{ background: CATEGORY_COLOR[s.cat] }}
              />
              {CATEGORY_LABEL[s.cat]} {formatBytes(s.size)}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
