import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import pkg from "./package.json";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  // 版本号从 package.json 注入，避免界面里硬编码忘更新
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
    // 未定稿大类功能的前端门控：与后端 build.rs 的 feature_bloatware 读同一环境变量，
    // 一个开关同时管前后端，默认关（隐藏入口、不渲染页面）。
    // @ts-expect-error process is a nodejs global
    __FEATURE_BLOATWARE__: JSON.stringify(process.env.DISKBUTLER_FEATURE_BLOATWARE === "1"),
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
