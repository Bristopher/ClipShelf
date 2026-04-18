import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { SettingsApp } from "./SettingsApp";
import { FirstRunApp } from "./FirstRunApp";
import "./index.css";

// Any uncaught error paints to the page — otherwise a broken secondary
// window just appears frozen with no way to see what went wrong.
function renderFatal(message: string, stack?: string) {
  const root = document.getElementById("root");
  if (!root) return;
  root.innerHTML = `<pre style="color:#fca5a5;background:#0a0a0a;margin:0;padding:20px;font-family:ui-monospace,monospace;font-size:12px;white-space:pre-wrap;height:100vh;overflow:auto;">${message}\n\n${stack || ""}</pre>`;
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
