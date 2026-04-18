import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { Minus, Square, X } from "lucide-react";
import { resetWindow } from "@/lib/commands";

const appWindow = getCurrentWindow();

type HoverId = "title" | "min" | "max" | "close";

export function TitleBar() {
  const [hovered, setHovered] = useState<HoverId | null>(null);
  const [version, setVersion] = useState<string>("");
  const hoverTimer = useRef<number | null>(null);

  useEffect(() => {
    getVersion().then(setVersion).catch(console.error);
  }, []);

  const startDrag = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button")) return;
    appWindow.startDragging();
  };

  const onEnter = (id: HoverId) => {
    if (hoverTimer.current) window.clearTimeout(hoverTimer.current);
    hoverTimer.current = window.setTimeout(() => setHovered(id), 350);
  };
  const onLeave = () => {
    if (hoverTimer.current) window.clearTimeout(hoverTimer.current);
    setHovered(null);
  };

  const buttonTip =
    hovered === "min" ? "Minimize"
    : hovered === "max" ? "Reset size & position"
    : hovered === "close" ? "Hide to tray"
    : null;

  return (
    <div
      onMouseDown={startDrag}
      className="relative h-7 flex items-center justify-between bg-secondary/80 border-b border-border select-none shrink-0 cursor-default"
    >
      <div className="flex items-center gap-2 pl-3 flex-1 relative">
        <span
          onMouseEnter={() => onEnter("title")}
          onMouseLeave={onLeave}
          className="text-[11px] font-semibold tracking-wide text-muted-foreground hover:text-foreground transition-colors"
        >
          GKey Mover
        </span>
        {hovered === "title" && version && (
          <div
            className="absolute top-full left-2 mt-1.5 pointer-events-none z-50"
            aria-hidden="true"
          >
            <div className="relative px-2.5 py-1 rounded-md bg-popover text-popover-foreground text-[11px] font-medium shadow-lg border border-border whitespace-nowrap animate-in fade-in-0 zoom-in-95 duration-150">
              v{version}
              <div className="absolute -top-1 left-3 w-2 h-2 bg-popover border-l border-t border-border rotate-45" />
            </div>
          </div>
        )}
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

        {buttonTip && (
          <div
            className="absolute top-full right-0 mt-1.5 mr-1 pointer-events-none z-50"
            aria-hidden="true"
          >
            <div className="relative px-2.5 py-1 rounded-md bg-popover text-popover-foreground text-[11px] font-medium shadow-lg border border-border whitespace-nowrap animate-in fade-in-0 zoom-in-95 duration-150">
              {buttonTip}
              <div className="absolute -top-1 right-3 w-2 h-2 bg-popover border-l border-t border-border rotate-45" />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
