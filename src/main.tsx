import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/tokens.css";

// 主题偏好：渲染前先应用用户上次的手动选择（auto/未设置 = 跟随系统，不写属性）
try {
  const saved = localStorage.getItem("diskbutler-theme");
  if (saved === "light" || saved === "dark") {
    document.documentElement.dataset.theme = saved;
  }
} catch {
  /* 读不到存储就跟随系统，不影响启动 */
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
