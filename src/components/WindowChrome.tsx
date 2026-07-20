import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import { Tip } from "@/components/ui/tip";
import logoUrl from "@/assets/gkey-logo.png";

const appWindow = getCurrentWindow();

interface WindowChromeProps {
  title: string;
  /** Return false to block hiding (e.g. unsaved changes). Default = hide. */
  onCloseRequest?: () => boolean | Promise<boolean>;
}

/** Themed title bar for secondary windows (settings, first-run).
 *  Drag, minimize, maximize-toggle, and close (hide). Matches main app. */
export function WindowChrome({ title, onCloseRequest }: WindowChromeProps) {
  const startDrag = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button")) return;
    appWindow.startDragging();
  };

  const handleClose = async () => {
    if (onCloseRequest) {
      const allow = await onCloseRequest();
      if (allow === false) return;
    }
    appWindow.hide();
  };

  return (
    <div
      onMouseDown={startDrag}
      className="h-8 flex items-center justify-between bg-title-bar border-b border-t-border select-none shrink-0 cursor-default"
    >
      <div className="flex items-center gap-2 pl-2.5 flex-1 pointer-events-none">
        <img src={logoUrl} alt="" className="h-4 w-4 rounded-sm" />
        <span className="text-[11px] font-semibold tracking-wide text-t-muted">
          {title}
        </span>
      </div>
      <div className="flex items-center h-full">
        <Tip text="Minimize" side="bottom" wrapperClass="h-full">
          <button
            onClick={() => appWindow.minimize()}
            className="h-full w-11 flex items-center justify-center text-t-muted hover:bg-hover hover:text-t-text transition-colors"
          >
            <Minus className="h-3.5 w-3.5" />
          </button>
        </Tip>
        <Tip text="Maximize" side="bottom" wrapperClass="h-full">
          <button
            onClick={() => appWindow.toggleMaximize()}
            className="h-full w-11 flex items-center justify-center text-t-muted hover:bg-hover hover:text-t-text transition-colors"
          >
            <Square className="h-3 w-3" />
          </button>
        </Tip>
        <Tip text="Close" side="bottom" align="right" wrapperClass="h-full">
          <button
            onClick={handleClose}
            className="h-full w-11 flex items-center justify-center text-t-muted hover:bg-red-600 hover:text-white transition-colors"
          >
            <X className="h-4 w-4" />
          </button>
        </Tip>
      </div>
    </div>
  );
}
