import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

/** 设置窗口入口（M1 桩；安装状态与诊断在 M2 填充——架构 §9.4）。 */
function Settings(): React.ReactElement {
  return (
    <div style={{ fontFamily: "system-ui, sans-serif", padding: 16, lineHeight: 1.7 }}>
      <h1 style={{ fontSize: 16, margin: "0 0 8px" }}>代码娘 · 设置（M1 桩）</h1>
      <p style={{ fontSize: 13, opacity: 0.7, margin: 0 }}>
        hooks 安装状态与诊断面板将在 M2 提供。
      </p>
    </div>
  );
}

const host = document.getElementById("app");
if (host) {
  createRoot(host).render(
    <StrictMode>
      <Settings />
    </StrictMode>,
  );
}
