import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";

const appWindow = getCurrentWindow();

export function TitleBar() {
  return (
    <div
      data-tauri-drag-region
      className="h-7 flex items-center justify-between border-b border-border select-none shrink-0"
      style={{ backgroundColor: "hsl(240 6% 12%)" }}
    >
      <div data-tauri-drag-region className="flex items-center gap-2 pl-3 flex-1">
        <span
          data-tauri-drag-region
          className="text-[11px] font-semibold tracking-wide"
          style={{ color: "hsl(0 0% 65%)" }}
        >
          Gkey Mover v2
        </span>
      </div>
      <div className="flex items-center h-full">
        <button
          onClick={() => appWindow.minimize()}
          className="h-full w-10 hover:bg-white/10 transition-colors flex items-center justify-center"
        >
          <Minus className="h-3.5 w-3.5" style={{ color: "hsl(0 0% 65%)" }} />
        </button>
        <button
          onClick={() => appWindow.toggleMaximize()}
          className="h-full w-10 hover:bg-white/10 transition-colors flex items-center justify-center"
        >
          <Square className="h-3 w-3" style={{ color: "hsl(0 0% 65%)" }} />
        </button>
        <button
          onClick={() => appWindow.hide()}
          className="h-full w-10 hover:bg-red-600 transition-colors flex items-center justify-center"
        >
          <X className="h-4 w-4" style={{ color: "hsl(0 0% 65%)" }} />
        </button>
      </div>
    </div>
  );
}
