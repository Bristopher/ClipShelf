import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { SettingsApp } from "./SettingsApp";
import { FirstRunApp } from "./FirstRunApp";
import "./index.css";

// Any uncaught error paints to the page — otherwise a broken secondary
// window just appears frozen with no way to see what went wrong. Includes
// Reload + Close buttons so the user can recover without killing the app.
function renderFatal(message: string, stack?: string) {
  const root = document.getElementById("root");
  if (!root) return;
  const escape = (s: string) =>
    s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  root.innerHTML = `
    <div style="background:#0a0a0a;color:#fca5a5;height:100vh;display:flex;flex-direction:column;font-family:ui-monospace,monospace;">
      <div style="display:flex;gap:8px;padding:10px;border-bottom:1px solid #333;background:#171717;">
        <button id="__err_back" style="background:#262626;color:#fafafa;border:1px solid #3f3f3f;padding:6px 12px;border-radius:6px;cursor:pointer;font-size:12px;">← Close window</button>
        <button id="__err_reload" style="background:#262626;color:#fafafa;border:1px solid #3f3f3f;padding:6px 12px;border-radius:6px;cursor:pointer;font-size:12px;">Reload</button>
      </div>
      <pre style="margin:0;padding:16px;font-size:12px;white-space:pre-wrap;flex:1;overflow:auto;">${escape(message)}\n\n${escape(stack || "")}</pre>
    </div>`;
  document
    .getElementById("__err_reload")
    ?.addEventListener("click", () => location.reload());
  document
    .getElementById("__err_back")
    ?.addEventListener("click", async () => {
      try {
        const mod = await import("@tauri-apps/api/window");
        await mod.getCurrentWindow().hide();
      } catch {
        window.close();
      }
    });
}
window.addEventListener("error", (e) => {
  renderFatal(e.message, e.error?.stack);
});
window.addEventListener("unhandledrejection", (e) => {
  const reason: any = e.reason;
  renderFatal(String(reason?.message || reason), reason?.stack);
});

// Tauri injects the window label into the webview; use it to decide which
// root component to render.
let label = "main";
try {
  label = getCurrentWindow().label || "main";
} catch {
  label = "main";
}

const Root =
  label === "settings" ? SettingsApp : label === "first-run" ? FirstRunApp : App;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
