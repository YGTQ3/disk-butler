import { useState } from "react";
import { Monitor, Sun, Moon } from "lucide-react";

/** 主题三态：跟随系统 → 浅色 → 深色 → 循环切换，选择持久化（auto = 移除属性回退跟随系统） */
type ThemeMode = "auto" | "light" | "dark";

const MODES: ThemeMode[] = ["auto", "light", "dark"];

const MODE_META: Record<ThemeMode, { label: string; icon: React.ReactNode }> = {
  auto: { label: "外观：跟随系统", icon: <Monitor size={20} /> },
  light: { label: "外观：浅色", icon: <Sun size={20} /> },
  dark: { label: "外观：深色", icon: <Moon size={20} /> },
};

function readStored(): ThemeMode {
  try {
    const v = localStorage.getItem("diskbutler-theme");
    if (v === "light" || v === "dark") return v;
  } catch {
    /* 忽略 */
  }
  return "auto";
}

export default function ThemeToggle() {
  const [mode, setMode] = useState<ThemeMode>(readStored);

  function cycle() {
    const next = MODES[(MODES.indexOf(mode) + 1) % MODES.length];
    setMode(next);
    try {
      if (next === "auto") {
        localStorage.removeItem("diskbutler-theme");
        delete document.documentElement.dataset.theme; // 回退到跟随系统
      } else {
        localStorage.setItem("diskbutler-theme", next);
        document.documentElement.dataset.theme = next;
      }
    } catch {
      /* 存储不可用时至少本次会话内切换生效 */
      if (next === "auto") delete document.documentElement.dataset.theme;
      else document.documentElement.dataset.theme = next;
    }
  }

  const meta = MODE_META[mode];
  return (
    <button
      onClick={cycle}
      title="点击切换：跟随系统 → 浅色 → 深色"
      className="flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left text-sm text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg)] hover:text-[var(--color-text-main)]"
    >
      {meta.icon}
      <span className="flex-1">{meta.label}</span>
    </button>
  );
}
