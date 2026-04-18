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

  return (
    <div
      onMouseDown={startDrag}
      className="relative h-7 flex items-center justify-between bg-title-bar border-b border-t-border select-none shrink-0 cursor-default"
    >
      <div className="flex items-center gap-2 pl-3 flex-1 relative">
        <span
          onMouseEnter={() => onEnter("title")}
          onMouseLeave={onLeave}
          className="text-[11px] font-semibold tracking-wide text-t-muted hover:text-t-text transition-colors"
        >
          GKey Mover
        </span>
        {hovered === "title" && version && (
          <Tooltip align="left-start" text={`v${version}`} />
        )}
      </div>
      <div className="flex items-center h-full">
        <BarButton
          active={hovered === "min"}
          tip="Minimize"
          onEnter={() => onEnter("min")}
          onLeave={onLeave}
          onClick={() => {
            onLeave();
            appWindow.minimize();
          }}
          hoverClass="hover:bg-hover hover:text-t-text"
        >
          <Minus className="h-3.5 w-3.5" />
        </BarButton>
        <BarButton
          active={hovered === "max"}
          tip="Reset size & position"
          onEnter={() => onEnter("max")}
          onLeave={onLeave}
          onClick={() => {
            onLeave();
            resetWindow().catch(console.error);
          }}
          hoverClass="hover:bg-hover hover:text-t-text"
        >
          <Square className="h-3 w-3" />
        </BarButton>
        <BarButton
          active={hovered === "close"}
          tip="Hide to tray"
          onEnter={() => onEnter("close")}
          onLeave={onLeave}
          onClick={() => {
            onLeave();
            appWindow.hide();
          }}
          hoverClass="hover:bg-red-600 hover:text-white"
        >
          <X className="h-4 w-4" />
        </BarButton>
      </div>
    </div>
  );
}

function BarButton({
  active,
  tip,
  onEnter,
  onLeave,
  onClick,
  hoverClass,
  children,
}: {
  active: boolean;
  tip: string;
  onEnter: () => void;
  onLeave: () => void;
  onClick: () => void;
  hoverClass: string;
  children: React.ReactNode;
}) {
  return (
    <div className="relative h-full">
      <button
        onMouseEnter={onEnter}
        onMouseLeave={onLeave}
        onClick={onClick}
        className={`h-full w-10 flex items-center justify-center text-t-muted transition-colors ${hoverClass}`}
      >
        {children}
      </button>
      {active && <Tooltip align="center" text={tip} />}
    </div>
  );
}

function Tooltip({
  text,
  align,
}: {
  text: string;
  align: "center" | "left-start";
}) {
  const positionClass =
    align === "center"
      ? "left-1/2 -translate-x-1/2"
      : "left-0";
  const arrowClass =
    align === "center" ? "left-1/2 -translate-x-1/2" : "left-3";
  return (
    <div
      className={`absolute top-full mt-1.5 pointer-events-none z-50 ${positionClass}`}
      aria-hidden="true"
    >
      <div className="relative px-2.5 py-1 rounded-md bg-popover text-popover-foreground text-[11px] font-medium shadow-lg border border-border whitespace-nowrap animate-in fade-in-0 zoom-in-95 duration-150">
        {text}
        <div
          className={`absolute -top-1 w-2 h-2 bg-popover border-l border-t border-border rotate-45 ${arrowClass}`}
        />
      </div>
    </div>
  );
}
