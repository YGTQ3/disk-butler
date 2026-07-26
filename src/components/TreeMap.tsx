import { useMemo, useRef, useState } from "react";
import { hierarchy, treemap, treemapSquarify } from "d3-hierarchy";
import { motion, AnimatePresence } from "framer-motion";
import { TreeNode, CATEGORY_COLOR, formatBytes } from "../types";

interface Props {
  node: TreeNode;
  /** 单击块：下钻（目录）或选中（文件） */
  onDrill: (node: TreeNode) => void;
  /** 悬停/点击时把节点告诉详情卡 */
  onSelect: (node: TreeNode) => void;
  width: number;
  height: number;
}

interface TooltipState {
  x: number;
  y: number;
  node: TreeNode;
}

/** 单个矩形块最小可见面积（px²），低于此值不渲染文字 */
const MIN_LABEL_AREA = 2800;

export default function TreeMap({ node, onDrill, onSelect, width, height }: Props) {
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const leaves = useMemo(() => {
    if (!node.children || node.children.length === 0) return [];
    // 只布局一层子项：下钻靠点击进入，视觉更干净（WizTree 式多层嵌套对普通人太吵）
    // 注意：根节点必须直接传引用，不能浅拷贝，否则 children 访问器的引用比较会失效
    const root = hierarchy<TreeNode>(
      node,
      (d) => (d === node ? d.children : undefined),
    )
      .sum((d) => (d === node ? 0 : d.size))
      .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));

    treemap<TreeNode>()
      .tile(treemapSquarify.ratio(1.4))
      .size([width, height])
      .paddingInner(4)
      .round(true)(root);

    return root.leaves().filter((l) => l.data !== node);
  }, [node, width, height]);

  if (leaves.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--color-text-secondary)]">
        这个目录里没有可显示的内容
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className="relative"
      style={{ width, height }}
      onMouseLeave={() => setTooltip(null)}
    >
      <AnimatePresence>
        {leaves.map((leaf) => {
          const d = leaf.data;
          const w = (leaf as any).x1 - (leaf as any).x0;
          const h = (leaf as any).y1 - (leaf as any).y0;
          if (w <= 0 || h <= 0) return null;
          const showLabel = w * h >= MIN_LABEL_AREA;
          const clickable = d.isDir && d.hasChildren && !d.path.endsWith("::__others__");
          return (
            <motion.div
              key={d.path}
              layout
              initial={{ opacity: 0, scale: 0.92 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.95 }}
              transition={{ duration: 0.3, ease: "easeOut" }}
              className={[
                "absolute overflow-hidden rounded-lg",
                clickable ? "cursor-pointer" : "cursor-default",
              ].join(" ")}
              style={{
                left: (leaf as any).x0,
                top: (leaf as any).y0,
                width: w,
                height: h,
                background: CATEGORY_COLOR[d.category],
              }}
              whileHover={{ filter: "brightness(1.08)", zIndex: 5 }}
              onMouseMove={(e) => {
                const rect = containerRef.current?.getBoundingClientRect();
                if (!rect) return;
                setTooltip({
                  x: e.clientX - rect.left,
                  y: e.clientY - rect.top,
                  node: d,
                });
              }}
              onClick={() => {
                onSelect(d);
                if (clickable) onDrill(d);
              }}
            >
              {showLabel && (
                <div className="pointer-events-none flex h-full flex-col justify-between p-2">
                  <div className="truncate text-xs font-medium text-white/95 drop-shadow-sm">
                    {d.name}
                  </div>
                  <div className="text-[11px] font-semibold text-white/85 drop-shadow-sm">
                    {formatBytes(d.size)}
                  </div>
                </div>
              )}
            </motion.div>
          );
        })}
      </AnimatePresence>

      {/* 悬停 Tooltip */}
      {tooltip && (
        <div
          className="pointer-events-none absolute z-20 max-w-72 rounded-xl bg-[var(--color-text-main)] px-3.5 py-2.5 text-white shadow-lg"
          style={{
            left: Math.min(tooltip.x + 14, width - 290),
            top: Math.min(tooltip.y + 14, height - 110),
          }}
        >
          <div className="text-[13px] font-semibold">{tooltip.node.friendlyName}</div>
          <div className="mt-0.5 text-xs opacity-75">
            {tooltip.node.name} · {formatBytes(tooltip.node.size)}
          </div>
          <div className="mt-1.5 text-xs leading-relaxed opacity-90">
            {tooltip.node.description}
          </div>
        </div>
      )}
    </div>
  );
}
