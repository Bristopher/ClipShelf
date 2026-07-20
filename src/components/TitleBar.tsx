import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { Minus, MousePointerClick, Skull, Square, X } from "lucide-react";
import { resetWindow, fullQuit } from "@/lib/commands";
import { Tip } from "@/components/ui/tip";
import logoUrl from "@/assets/gkey-logo.png";

const appWindow = getCurrentWindow();

type HoverId = "title" | "min" | "max" | "close";

export function TitleBar() {
  const [hovered, setHovered] = useState<HoverId | null>(null);
  const [version, setVersion] = useState<string>("");
  const [ctrlHeld, setCtrlHeld] = useState(false);
  const hoverTimer = useRef<number | null>(null);

  useEffect(() => {
    getVersion().then(setVersion).catch(console.error);
  }, []);

  // Backend hold-to-click-through state — show a badge while clicks are
  // falling through the window so it doesn't read as "app stopped working".
  const [clickThrough, setClickThrough] = useState(false);
  useEffect(() => {
    const unlisten = listen<{ active: boolean }>("click-through-changed", (e) => {
      setClickThrough(e.payload.active);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // While the close button is hovered, watch for Ctrl key state. We toggle
  // the icon (X ↔ skull) and tooltip text based on whether Ctrl is held so
  // the user knows clicking will fully quit instead of hiding to tray.
  useEffect(() => {
    if (hovered !== "close") {
      setCtrlHeld(false);
      return;
    }
    const onDown = (e: KeyboardEvent) => {
      if (e.key === "Control") setCtrlHeld(true);
    };
    const onUp = (e: KeyboardEvent) => {
      if (e.key === "Control") setCtrlHeld(false);
    };
    document.addEventListener("keydown", onDown);
    document.addEventListener("keyup", onUp);
    return () => {
      document.removeEventListener("keydown", onDown);
      document.removeEventListener("keyup", onUp);
    };
  }, [hovered]);

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
      <div className="flex items-center gap-2 pl-2 flex-1 relative">
        <img
          src={logoUrl}
          alt=""
          className="h-4 w-4 rounded-sm pointer-events-none shrink-0"
        />
        <span
          onMouseEnter={() => onEnter("title")}
          onMouseLeave={onLeave}
          className="text-[11px] font-semibold tracking-wide text-t-muted hover:text-t-text transition-colors"
        >
          ClipShelf
        </span>
        {hovered === "title" && version && (
          <Tooltip align="left-start" text={`v${version}`} />
        )}
        {clickThrough && (
          <Tip
            text="Click-through active — clicks pass through the window"
            side="bottom"
            align="left"
          >
            <span className="flex items-center gap-1 text-[10px] text-t-muted">
              <MousePointerClick className="h-3 w-3" />
              click-through
            </span>
          </Tip>
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
        <CloseButton
          active={hovered === "close"}
          // During click-through the close button is the one clickable
          // carve-out (backend drops WS_EX_TRANSPARENT over its rect), so
          // it shows the skull the whole time the modifier is held — even
          // when the configured key isn't Ctrl and the game has focus.
          ctrlHeld={ctrlHeld || clickThrough}
          onEnter={() => {
            // We do NOT use the 350ms delay here so the Ctrl-detection
            // listener arms immediately on hover.
            if (hoverTimer.current) window.clearTimeout(hoverTimer.current);
            setHovered("close");
          }}
          onLeave={onLeave}
          onClick={(e) => {
            onLeave();
            if (e.ctrlKey || clickThrough) {
              fullQuit().catch(console.error);
            } else {
              appWindow.hide();
            }
          }}
        />
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

function CloseButton({
  active,
  ctrlHeld,
  onEnter,
  onLeave,
  onClick,
}: {
  active: boolean;
  ctrlHeld: boolean;
  onEnter: () => void;
  onLeave: () => void;
  onClick: (e: React.MouseEvent) => void;
}) {
  const hoverClass = ctrlHeld
    ? "hover:bg-red-700 hover:text-white"
    : "hover:bg-red-600 hover:text-white";
  const baseClass = ctrlHeld ? "bg-red-600/40 text-white" : "text-t-muted";
  return (
    <div className="relative h-full">
      <button
        onMouseEnter={onEnter}
        onMouseLeave={onLeave}
        onClick={onClick}
        className={`h-full w-10 flex items-center justify-center transition-colors ${baseClass} ${hoverClass}`}
      >
        <div className="relative h-4 w-4">
          <X
            className={`absolute inset-0 h-4 w-4 transition-all duration-200 ${
              ctrlHeld ? "scale-0 rotate-90 opacity-0" : "scale-100 opacity-100"
            }`}
          />
          <Skull
            className={`absolute inset-0 h-4 w-4 transition-all duration-200 ${
              ctrlHeld ? "scale-100 opacity-100" : "scale-0 -rotate-90 opacity-0"
            }`}
          />
        </div>
      </button>
      {active && <CloseTooltip ctrlHeld={ctrlHeld} />}
    </div>
  );
}

function CloseTooltip({ ctrlHeld }: { ctrlHeld: boolean }) {
  return (
    <div
      className="absolute top-full mt-1.5 right-0 pointer-events-none z-50"
      aria-hidden="true"
    >
      <div className="relative px-2.5 py-1.5 rounded-md bg-popover text-popover-foreground shadow-lg border border-border whitespace-nowrap animate-in fade-in-0 zoom-in-95 duration-150">
        <p className="text-[11px] font-semibold leading-tight">
          {ctrlHeld ? "Quit ClipShelf" : "Hide to tray"}
        </p>
        <p className="text-[9px] text-muted-foreground leading-tight mt-0.5">
          {ctrlHeld
            ? "Fully exit the application"
            : "Hold Ctrl + click to fully quit"}
        </p>
        <div className="absolute -top-1 right-3 w-2 h-2 bg-popover border-l border-t border-border rotate-45" />
      </div>
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
