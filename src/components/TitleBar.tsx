import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";

const appWindow = getCurrentWindow();

export function TitleBar() {
  const startDrag = (e: React.MouseEvent) => {
    // Only drag from the bar itself, not from buttons
    if ((e.target as HTMLElement).closest("button")) return;
    appWindow.startDragging();
  };

  return (
    <div
      onMouseDown={startDrag}
      className="h-7 flex items-center justify-between bg-secondary/80 border-b border-border select-none shrink-0 cursor-default"
    >
      <div className="flex items-center gap-2 pl-3 flex-1 pointer-events-none">
        <span className="text-[11px] font-semibold tracking-wide text-muted-foreground">
          Gkey Mover v2
        </span>
      </div>
      <div className="flex items-center h-full">
        <button
          onClick={() => appWindow.minimize()}
          className="h-full w-10 hover:bg-white/10 transition-colors flex items-center justify-center"
        >
          <Minus className="h-3.5 w-3.5 text-muted-foreground" />
        </button>
        <button
          onClick={() => appWindow.toggleMaximize()}
          className="h-full w-10 hover:bg-white/10 transition-colors flex items-center justify-center"
        >
          <Square className="h-3 w-3 text-muted-foreground" />
        </button>
        <button
          onClick={() => appWindow.hide()}
          className="h-full w-10 hover:bg-red-600 transition-colors flex items-center justify-center"
        >
          <X className="h-4 w-4 text-muted-foreground" />
        </button>
      </div>
    </div>
  );
}
