import { useEffect, useRef, useState } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { revealInExplorer, openFolder } from "@/lib/commands";
import { errorMessage, toastError } from "@/lib/toast";
import type { LogEntry } from "@/types";

function categoryColor(category: string, level: string): string {
  if (level === "error") return "text-red-400";
  if (level === "warning") return "text-red-300";
  if (category === "file-created") return "text-green-400";
  if (category === "file-moved" || category === "file-renamed") return "text-purple-400";
  if (category === "watcher" || category === "obs") return "text-yellow-400";
  if (level === "success") return "text-green-400";
  return "text-muted-foreground";
}

interface EventLogProps {
  entries: LogEntry[];
}

/**
 * Log entries that reference a file are clickable:
 *   Click        → reveal in Explorer (file selected)
 *   Ctrl + Click → open in the default video player
 * A custom tooltip (same visual language as the title-bar tooltips) appears
 * after a short hover delay to teach both actions.
 */
export function EventLog({ entries }: EventLogProps) {
  const bottomRef = useRef<HTMLDivElement>(null);
  const [hoverIdx, setHoverIdx] = useState<number | null>(null);
  const [ctrlHeld, setCtrlHeld] = useState(false);
  const hoverTimer = useRef<number | null>(null);
  // Only auto-scroll while the user is already at (or near) the bottom —
  // a new entry shouldn't yank them down mid-read of an older one.
  // onScrollCapture because scroll events don't bubble out of the
  // ScrollArea viewport.
  const nearBottomRef = useRef(true);

  const handleScrollCapture = (e: React.UIEvent<HTMLDivElement>) => {
    const el = e.target as HTMLElement;
    if (!el || typeof el.scrollHeight !== "number") return;
    nearBottomRef.current =
      el.scrollHeight - el.scrollTop - el.clientHeight < 40;
  };

  useEffect(() => {
    if (nearBottomRef.current) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [entries]);

  // Track Ctrl while a clickable entry is hovered so the tooltip highlights
  // the action the click will actually perform.
  useEffect(() => {
    if (hoverIdx === null) {
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
  }, [hoverIdx]);

  const onEnter = (i: number) => {
    if (hoverTimer.current) window.clearTimeout(hoverTimer.current);
    hoverTimer.current = window.setTimeout(() => setHoverIdx(i), 400);
  };
  const onLeave = () => {
    if (hoverTimer.current) window.clearTimeout(hoverTimer.current);
    setHoverIdx(null);
  };

  const handleClick = (entry: LogEntry, e: React.MouseEvent) => {
    if (!entry.path) return;
    onLeave();
    if (e.ctrlKey) {
      // openFolder is opener::open — for a file path that means the
      // default video player.
      openFolder(entry.path).catch((err) => toastError(errorMessage(err)));
    } else {
      revealInExplorer(entry.path).catch((err) => toastError(errorMessage(err)));
    }
  };

  return (
    <ScrollArea className="flex-1 px-3 py-2" onScrollCapture={handleScrollCapture}>
      {entries.length === 0 ? (
        <p className="text-sm text-muted-foreground italic pt-4 text-center">
          Waiting for events...
        </p>
      ) : (
        <div className="space-y-0.5">
          {entries.map((entry, i) => (
            <div key={i} className="flex gap-2 text-xs leading-5 font-mono">
              <span className="text-muted-foreground shrink-0">
                {entry.timestamp}
              </span>
              {entry.path ? (
                <span className="relative">
                  <span
                    onClick={(e) => handleClick(entry, e)}
                    onMouseEnter={() => onEnter(i)}
                    onMouseLeave={onLeave}
                    className={`${categoryColor(entry.category, entry.level)} cursor-pointer underline-offset-2 hover:underline hover:brightness-125`}
                  >
                    {entry.message}
                  </span>
                  {hoverIdx === i && <ClipTooltip ctrlHeld={ctrlHeld} />}
                </span>
              ) : (
                <span className={categoryColor(entry.category, entry.level)}>
                  {entry.message}
                </span>
              )}
            </div>
          ))}
          <div ref={bottomRef} />
        </div>
      )}
    </ScrollArea>
  );
}

function ClipTooltip({ ctrlHeld }: { ctrlHeld: boolean }) {
  return (
    <div
      className="absolute bottom-full mb-1.5 left-0 pointer-events-none z-50"
      aria-hidden="true"
    >
      <div className="relative px-2.5 py-1.5 rounded-md bg-popover text-popover-foreground shadow-lg border border-border whitespace-nowrap font-sans animate-in fade-in-0 zoom-in-95 duration-150">
        <p
          className={`text-[10px] leading-tight ${
            !ctrlHeld ? "font-semibold" : "text-muted-foreground"
          }`}
        >
          Click — reveal in Explorer
        </p>
        <p
          className={`text-[10px] leading-tight mt-0.5 ${
            ctrlHeld ? "font-semibold" : "text-muted-foreground"
          }`}
        >
          Ctrl + Click — play clip
        </p>
        <div className="absolute -bottom-1 left-4 w-2 h-2 bg-popover border-r border-b border-border rotate-45" />
      </div>
    </div>
  );
}
