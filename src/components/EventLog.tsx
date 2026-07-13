import { useEffect, useMemo, useRef, useState } from "react";
import { Search, X } from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Input } from "@/components/ui/input";
import { revealInExplorer, openFolder } from "@/lib/commands";
import { useWatcherStatus } from "@/hooks/useWatcherStatus";
import { errorMessage, toastError } from "@/lib/toast";
import { EntryContextMenu, type ContextMenuState } from "@/components/EntryContextMenu";
import type { AppConfig, LogEntry } from "@/types";

function categoryColor(category: string, level: string): string {
  if (level === "error") return "text-red-400";
  if (level === "warning") return "text-red-300";
  if (category === "file-created") return "text-green-400";
  if (category === "file-moved" || category === "file-renamed") return "text-purple-400";
  if (category === "watcher" || category === "obs") return "text-yellow-400";
  if (level === "success") return "text-green-400";
  return "text-muted-foreground";
}

const LEVELS = ["all", "info", "success", "warning", "error"] as const;
type LevelFilter = (typeof LEVELS)[number];

interface EventLogProps {
  entries: LogEntry[];
  config: AppConfig;
  /** Filter bar visibility — toggled by Ctrl+F (App) or the X button here. */
  filterOpen: boolean;
  onCloseFilter: () => void;
}

/**
 * Log entries that reference a file are clickable:
 *   Click        → reveal in Explorer (file selected)
 *   Ctrl + Click → open in the default video player
 *   Right-click  → context menu (copy path/name, reveal, play)
 * A custom tooltip (same visual language as the title-bar tooltips) appears
 * after a short hover delay to teach both actions.
 */
export function EventLog({ entries, config, filterOpen, onCloseFilter }: EventLogProps) {
  const bottomRef = useRef<HTMLDivElement>(null);
  const [hoverIdx, setHoverIdx] = useState<number | null>(null);
  const [ctrlHeld, setCtrlHeld] = useState(false);
  const hoverTimer = useRef<number | null>(null);
  const [query, setQuery] = useState("");
  const [level, setLevel] = useState<LevelFilter>("all");
  const [menu, setMenu] = useState<ContextMenuState | null>(null);
  const filterInputRef = useRef<HTMLInputElement>(null);
  // Only auto-scroll while the user is already at (or near) the bottom —
  // a new entry shouldn't yank them down mid-read of an older one.
  // onScrollCapture because scroll events don't bubble out of the
  // ScrollArea viewport.
  const nearBottomRef = useRef(true);

  const filtering = filterOpen && (query.trim() !== "" || level !== "all");
  const shown = useMemo(() => {
    if (!filtering) return entries;
    const q = query.trim().toLowerCase();
    return entries.filter(
      (e) =>
        (level === "all" || e.level === level) &&
        (q === "" || e.message.toLowerCase().includes(q)),
    );
  }, [entries, filtering, query, level]);

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
  }, [shown]);

  useEffect(() => {
    if (filterOpen) {
      requestAnimationFrame(() => filterInputRef.current?.focus());
    }
  }, [filterOpen]);

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

  // Context menu closes on any click elsewhere or Esc.
  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMenu(null);
    };
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", onKey);
    };
  }, [menu]);

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

  const handleContextMenu = (entry: LogEntry, e: React.MouseEvent) => {
    if (!entry.path) return;
    e.preventDefault();
    onLeave();
    setMenu({ x: e.clientX, y: e.clientY, path: entry.path! });
  };

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      {filterOpen && (
        <div className="px-3 pt-2 pb-1.5 border-b border-t-border flex items-center gap-2">
          <Search className="h-3.5 w-3.5 text-t-muted shrink-0" />
          <Input
            ref={filterInputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.preventDefault();
                onCloseFilter();
              }
            }}
            placeholder="Filter log..."
            className="h-6 text-xs flex-1 min-w-0"
          />
          <div className="flex items-center gap-1">
            {LEVELS.map((l) => (
              <button
                key={l}
                onClick={() => setLevel(l)}
                className={`px-1.5 py-0.5 rounded text-[10px] capitalize border ${
                  level === l
                    ? "border-t-border bg-panel text-t-text"
                    : "border-transparent text-t-muted hover:text-t-text"
                }`}
              >
                {l}
              </button>
            ))}
          </div>
          <span className="text-[10px] text-t-muted tabular-nums shrink-0">
            {shown.length} / {entries.length}
          </span>
          <button
            onClick={onCloseFilter}
            title="Close filter (Esc)"
            className="text-t-muted hover:text-t-text"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      )}
      <ScrollArea className="flex-1 px-3 py-2" onScrollCapture={handleScrollCapture}>
        {shown.length === 0 ? (
          entries.length > 0 ? (
            <p className="text-sm text-muted-foreground italic pt-4 text-center">
              No entries match the filter
            </p>
          ) : (
            <EmptyState config={config} />
          )
        ) : (
          <div className="space-y-0.5">
            {shown.map((entry, i) => (
              <div key={i} className="flex gap-2 text-xs leading-5 font-mono">
                <span className="text-muted-foreground shrink-0">
                  {entry.timestamp}
                </span>
                {entry.path ? (
                  <span className="relative">
                    <span
                      onClick={(e) => handleClick(entry, e)}
                      onContextMenu={(e) => handleContextMenu(entry, e)}
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
      {menu && <EntryContextMenu menu={menu} onClose={() => setMenu(null)} />}
    </div>
  );
}

/** Empty log: show what the app is doing instead of a bare "waiting". */
function EmptyState({ config }: { config: AppConfig }) {
  const watcher = useWatcherStatus();
  const statusLabel =
    watcher.status === "running"
      ? "Watching for new clips"
      : watcher.status === "paused"
        ? "Watching is paused"
        : "Watcher is stopped";
  return (
    <div className="pt-4 text-center space-y-1.5">
      <p className="text-sm text-muted-foreground italic">Waiting for clips...</p>
      {config.videos_folder ? (
        <>
          <p className="text-[11px] text-t-muted font-mono truncate px-4" title={config.videos_folder}>
            {statusLabel}: {config.videos_folder}
          </p>
          <p className="text-[11px] text-t-muted">
            {config.save_clip_bind
              ? `Press ${config.save_clip_bind} in-game to save a clip — it'll appear here.`
              : "Save a clip in OBS/ShadowPlay and it'll appear here."}
            {" "}You can also drag a video onto this window.
          </p>
        </>
      ) : (
        <p className="text-[11px] text-t-muted">
          No clips folder configured — open Settings to pick one.
        </p>
      )}
    </div>
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
