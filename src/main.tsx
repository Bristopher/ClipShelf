import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { SettingsApp } from "./SettingsApp";
import "./index.css";

// Tauri injects the window label into the webview; use it to decide which
// root component to render. Avoids URL-param routing which Vite's dev
// server doesn't serve cleanly across different paths.
let isSettings = false;
try {
  isSettings = getCurrentWindow().label === "settings";
} catch {
  isSettings = false;
}

const Root = isSettings ? SettingsApp : App;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
