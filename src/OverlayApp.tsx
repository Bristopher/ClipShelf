import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

// Placeholder shell for the in-game overlay window. Task 6 replaces this
// with the real G-key feedback UI (recent clip, hotkey flash, etc.) — this
// task only proves the window itself is transparent, non-activating, and
// positionable via overlay::show/hide.
//
// The overlay window is `transparent(true)` + `decorations(false)` at the
// Tauri level, but `body` in index.css sets `@apply bg-background` (an
// opaque theme color) so every OTHER window paints a solid backdrop. That
// rule would otherwise paint this window's whole viewport opaque too, so
// this component overrides background to transparent on its own root
// (inline styles, not a CSS file edit — keeps the override scoped to this
// window only).
export function OverlayApp() {
  useEffect(() => {
    // index.css's `body { @apply bg-background }` paints an opaque color
    // that fills the whole (Tauri-transparent) window surface — override it
    // here rather than editing the shared stylesheet, so no other window is
    // affected.
    const prevBodyBg = document.body.style.background;
    const prevHtmlBg = document.documentElement.style.background;
    document.body.style.background = "transparent";
    document.documentElement.style.background = "transparent";
    return () => {
      document.body.style.background = prevBodyBg;
      document.documentElement.style.background = prevHtmlBg;
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<Record<string, unknown>>("overlay-key", (e) => {
      console.log("overlay-key", e.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "transparent",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <div className="bg-black/85 rounded-xl border border-white/10 text-white p-4">
        Overlay
      </div>
    </div>
  );
}
