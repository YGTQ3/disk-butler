import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/tokens.css";

// 禁用 WebView 默认右键菜单（浏览器那套返回/刷新/检查）；
// 仅在输入框等可编辑元素保留系统菜单，方便复制/粘贴。
document.addEventListener("contextmenu", (e) => {
  const el = e.target as HTMLElement | null;
  if (!el || !el.closest('input, textarea, [contenteditable="true"]')) {
    e.preventDefault();
  }
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
