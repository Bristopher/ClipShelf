import { useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import { resetWindow } from "@/lib/commands";

const appWindow = getCurrentWindow();

type BtnId = "min" | "max" | "close";

const TOOLTIPS: Record<BtnId, string> = {
  min: "Minimize",
  max: "Reset size & position",
  close: "Hide to tray",
};

export function TitleBar() {
  const [hovered, setHovered] = useState<BtnId | null>(null);
  const hoverTimer = useRef<number | null>(null);

  const startDrag = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button")) return;
    appWindow.startDragging();
  };

  const onEnter = (id: BtnId) => {
    if (hoverTimer.current) window.clearTimeout(hoverTimer.current);
    hoverTimer.current = window.setTimeout(() => setHovered(id), 350);
  };
  const onLeave = () => {
    if (hoverTimer.current) window.clearTimeout(hoverTimer.current);
    setHovered(null);
  };

  return (
    <div
      onMouseDown={startDrag}
      className="relative h-7 flex items-center justify-between bg-secondary/80 border-b border-border select-none shrink-0 cursor-default"
    >
      <div className="flex items-center gap-2 pl-3 flex-1 pointer-events-none">
        <span className="text-[11px] font-semibold tracking-wide text-muted-foreground">
          Gkey Mover v2
        </span>
      </div>
      <div className="flex items-center h-full relative">
        <button
          onMouseEnter={() => onEnter("min")}
          onMouseLeave={onLeave}
          onClick={() => {
            onLeave();
            appWindow.minimize();
          }}
          className="h-full w-10 flex items-center justify-center text-muted-foreground hover:bg-white/15 hover:text-foreground transition-colors"
        >
          <Minus className="h-3.5 w-3.5" />
        </button>
        <button
          onMouseEnter={() => onEnter("max")}
          onMouseLeave={onLeave}
          onClick={() => {
            onLeave();
            resetWindow().catch(console.error);
          }}
          className="h-full w-10 flex items-center justify-center text-muted-foreground hover:bg-white/15 hover:text-foreground transition-colors"
        >
          <Square className="h-3 w-3" />
        </button>
        <button
          onMouseEnter={() => onEnter("close")}
          onMouseLeave={onLeave}
          onClick={() => {
            onLeave();
            appWindow.hide();
          }}
          className="h-full w-10 flex items-center justify-center text-muted-foreground hover:bg-red-600 hover:text-white transition-colors"
        >
          <X className="h-4 w-4" />
        </button>

        {hovered && (
          <div
            className="absolute top-full right-0 mt-1.5 mr-1 pointer-events-none z-50"
            aria-hidden="true"
          >
            <div className="relative px-2.5 py-1 rounded-md bg-popover text-popover-foreground text-[11px] font-medium shadow-lg border border-border whitespace-nowrap animate-in fade-in-0 zoom-in-95 duration-150">
              {TOOLTIPS[hovered]}
              <div className="absolute -top-1 right-3 w-2 h-2 bg-popover border-l border-t border-border rotate-45" />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
