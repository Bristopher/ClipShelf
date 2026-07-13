import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { getHistory, editHistoryGame, restoreLog, revealInExplorer } from "@/lib/commands";
import { EVENTS } from "@/lib/events";
import { EntryContextMenu, type ContextMenuState } from "@/components/EntryContextMenu";
import { errorMessage, toastError, toastSuccess } from "@/lib/toast";
import type { HistoryEntry, LogEntry } from "@/types";

interface HistoryViewProps {
  /** Toggle back to the live event log (History button / Esc). */
  onClose: () => void;
  onRestore: (entries: LogEntry[]) => void;
  dayRolloverHour: number;
}

type ViewMode = "today" | "all";

const EVENT_BADGES: Record<string, { label: string; className: string }> = {
  created: { label: "New", className: "text-green-400" },
  moved: { label: "Moved", className: "text-purple-400" },
  renamed: { label: "Renamed", className: "text-purple-400" },
  undone: { label: "Undone", className: "text-amber-400" },
  game_edited: { label: "Game edited", className: "text-blue-400" },
  rated: { label: "Rated", className: "text-t-muted" },
  labeled: { label: "Labeled", className: "text-t-muted" },
  described: { label: "Described", className: "text-t-muted" },
};

function badgeFor(event: string) {
  return EVENT_BADGES[event] ?? { label: event, className: "text-t-muted" };
}

/** Last path segment of the parent directory — the destination folder name for a moved row. */
function destFolder(path: string): string {
  const parent = path.replace(/[\\/][^\\/]*$/, "");
  const seg = parent.match(/[^\\/]+$/);
  return seg ? seg[0] : "";
}

function fmtTime(ts: string): string {
  const d = new Date(ts);
  return Number.isNaN(d.getTime()) ? ts : d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function fmtDay(day: string): string {
  const d = new Date(`${day}T00:00:00`);
  return Number.isNaN(d.getTime())
    ? day
    : d.toLocaleDateString([], { weekday: "short", month: "short", day: "numeric" });
}

/**
 * Row identity for React keys + edit-state targeting. A clip path recurs
 * across events (created → game_edited on the same clip is a normal flow)
 * and ts alone isn't guaranteed unique, so the group-local render index
 * disambiguates. Grouping is deterministic per fetch, so the key is stable
 * until the next refetch — long enough for an edit interaction.
 */
function rowKey(e: HistoryEntry, index: number): string {
  return `${e.path}|${e.ts}|${index}`;
}

/** Context-menu state: shared x/y/path plus the exact row it was opened on. */
interface HistoryMenuState extends ContextMenuState {
  rowKey: string;
  entry: HistoryEntry;
}

/** Distinct physical clips in a set of rows (events of one clip share clipId). */
function distinctClips(rows: HistoryEntry[]): number {
  return new Set(rows.map((e) => e.clipId)).size;
}

/**
 * Group entries by `game ?? "No game detected"`, groups sorted by DISTINCT
 * clip count desc. Because the backend reconciles game per clip identity, an
 * edited clip's whole history lands in one group — never split across games.
 */
function groupByGame(entries: HistoryEntry[]): [string, HistoryEntry[]][] {
  const map = new Map<string, HistoryEntry[]>();
  for (const e of entries) {
    const key = e.game ?? "No game detected";
    const bucket = map.get(key);
    if (bucket) bucket.push(e);
    else map.set(key, [e]);
  }
  return [...map.entries()].sort((a, b) => distinctClips(b[1]) - distinctClips(a[1]));
}

function groupByDay(entries: HistoryEntry[]): [string, HistoryEntry[]][] {
  const map = new Map<string, HistoryEntry[]>();
  for (const e of entries) {
    const bucket = map.get(e.day);
    if (bucket) bucket.push(e);
    else map.set(e.day, [e]);
  }
  return [...map.entries()].sort((a, b) => (a[0] < b[0] ? 1 : a[0] > b[0] ? -1 : 0));
}

/**
 * Full-pane history view — swapped into the main area in place of the live
 * event log while the BottomBar History toggle is on. Fetches on mount and
 * whenever a new log entry lands (so clips saved while it's open appear
 * live), with a Today/All toggle and per-row context menu / game editing.
 * Esc returns to the live log when no context menu or edit is open.
 */
export function HistoryView({ onClose, onRestore, dayRolloverHour }: HistoryViewProps) {
  const [view, setView] = useState<ViewMode>("today");
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [menu, setMenu] = useState<HistoryMenuState | null>(null);
  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  // Live mirrors so the mount-keyed listeners see current state without
  // re-registering on every change.
  const menuRef = useRef<HistoryMenuState | null>(null);
  menuRef.current = menu;
  const viewRef = useRef<ViewMode>(view);
  viewRef.current = view;
  // Set when a mousedown just closed the context menu — the click that
  // follows must NOT fire the row's reveal action.
  const menuJustClosedRef = useRef(false);

  const load = (full: boolean) => {
    getHistory(full)
      .then(setEntries)
      .catch((e) => toastError(errorMessage(e)));
  };

  // Initial fetch + live refresh: every backend action that appears in the
  // event log also lands in history, so LOG_ENTRY is the refresh signal.
  useEffect(() => {
    load(viewRef.current === "all");
    const unlisten = listen(EVENTS.LOG_ENTRY, () => {
      load(viewRef.current === "all");
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Esc closes the context menu first (its own [menu] effect handles
      // that); only leave the history view when no menu is open. Edit-input
      // Esc stops propagation before reaching here.
      if (e.key === "Escape" && !menuRef.current) onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Context menu closes on any mousedown elsewhere or Esc — same idiom as
  // EventLog's [menu]-keyed effect. The EntryContextMenu itself swallows
  // mousedown inside it so item clicks still land.
  useEffect(() => {
    if (!menu) return;
    const close = () => {
      menuJustClosedRef.current = true;
      setMenu(null);
    };
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

  const switchView = (v: ViewMode) => {
    setView(v);
    setEditingKey(null);
    load(v === "all");
  };

  const handleRestore = () => {
    restoreLog()
      .then((restored) => {
        onRestore(restored);
        // The restored log is the point — flip back to the live view.
        onClose();
      })
      .catch((e) => toastError(`Restore failed: ${errorMessage(e)}`));
  };

  const saveEdit = (e: HistoryEntry, remember: boolean) => {
    const game = editValue.trim();
    if (!game) {
      toastError("Game name cannot be empty");
      return;
    }
    editHistoryGame(e.path, game, e.exe ?? null, remember)
      .then(() => {
        toastSuccess("Game updated");
        setEditingKey(null);
        load(view === "all");
      })
      .catch((err) => toastError(errorMessage(err)));
  };

  const renderRow = (e: HistoryEntry, index: number) => {
    const key = rowKey(e, index);
    const badge = badgeFor(e.event);
    const editing = editingKey === key;
    return (
      <div key={key} className="rounded px-1.5 py-1 hover:bg-hover">
        <div
          className="flex items-center gap-2 text-xs cursor-pointer"
          onClick={() => {
            // The click that closed a context menu must not also reveal.
            if (menuJustClosedRef.current) {
              menuJustClosedRef.current = false;
              return;
            }
            revealInExplorer(e.path).catch((err) => toastError(errorMessage(err)));
          }}
          onContextMenu={(ev) => {
            ev.preventDefault();
            setMenu({ x: ev.clientX, y: ev.clientY, path: e.path, rowKey: key, entry: e });
          }}
        >
          <span className={`shrink-0 w-20 ${badge.className}`}>{badge.label}</span>
          <span className="flex-1 min-w-0 truncate text-t-text" title={e.path}>
            {e.filename}
          </span>
          {e.event === "moved" && (
            <span className="shrink-0 text-t-muted truncate max-w-[8rem]" title={destFolder(e.path)}>
              → {destFolder(e.path)}
            </span>
          )}
          <span className="shrink-0 text-t-muted tabular-nums">{fmtTime(e.ts)}</span>
        </div>
        {editing && (
          <div
            className="flex items-center gap-1.5 mt-1 pl-[5.5rem]"
            onClick={(ev) => ev.stopPropagation()}
          >
            <Input
              autoFocus
              value={editValue}
              onChange={(ev) => setEditValue(ev.target.value)}
              onKeyDown={(ev) => {
                if (ev.key === "Escape") {
                  // Cancel the edit only — don't let the document-level
                  // Esc listener also close the whole view.
                  ev.stopPropagation();
                  setEditingKey(null);
                }
                if (ev.key === "Enter") saveEdit(e, false);
              }}
              className="h-6 text-xs flex-1 min-w-0"
              placeholder="Game name"
            />
            <Button
              size="xs"
              variant="outline"
              className="h-6 text-[10px]"
              onClick={() => saveEdit(e, false)}
            >
              Save
            </Button>
            <Button
              size="xs"
              variant="outline"
              className="h-6 text-[10px]"
              disabled={!e.exe}
              title={e.exe ? undefined : "No exe recorded for this clip"}
              onClick={() => saveEdit(e, true)}
            >
              Save &amp; Remember
            </Button>
          </div>
        )}
      </div>
    );
  };

  const renderGameGroups = (list: HistoryEntry[]) =>
    groupByGame(list).map(([game, rows]) => {
      const clips = distinctClips(rows);
      return (
      <div key={game} className="mb-2">
        <p className="text-[10px] font-semibold text-t-muted px-1.5 py-0.5">
          {game} — {clips} clip{clips === 1 ? "" : "s"}
        </p>
        <div className="space-y-0.5">{rows.map(renderRow)}</div>
      </div>
      );
    });

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      <div className="flex items-center justify-between px-3 pt-2 pb-1.5 border-b border-t-border">
        <p className="text-[11px] font-semibold text-t-text">History</p>
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-t-muted">{entries?.length ?? 0} entries</span>
          <div className="flex items-center rounded border border-t-border overflow-hidden">
            {(["today", "all"] as const).map((v) => (
              <button
                key={v}
                onClick={() => switchView(v)}
                className={`px-2 py-0.5 text-[10px] capitalize ${
                  view === v ? "bg-hover text-t-text" : "text-t-muted hover:text-t-text"
                }`}
              >
                {v}
              </button>
            ))}
          </div>
          <button
            onClick={onClose}
            title="Back to live log (Esc)"
            className="text-[10px] text-t-muted hover:text-t-text px-1.5 py-0.5 rounded border border-t-border"
          >
            Back to log
          </button>
        </div>
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto px-2 py-1">
        {entries === null ? (
          <p className="text-[10px] text-t-muted px-1.5 py-2">Loading...</p>
        ) : entries.length === 0 ? (
          <p className="text-[10px] text-t-muted px-1.5 py-2 italic">
            {view === "today" ? "No clips yet today." : "No history yet."}
          </p>
        ) : view === "today" ? (
          renderGameGroups(entries)
        ) : (
          groupByDay(entries).map(([day, rows]) => {
            const clips = distinctClips(rows);
            return (
            <div key={day} className="mb-3">
              <p className="text-[10px] font-semibold text-t-text px-1.5 py-0.5 border-b border-t-border">
                {fmtDay(day)} — {clips} clip{clips === 1 ? "" : "s"}
              </p>
              {renderGameGroups(rows)}
            </div>
            );
          })
        )}
      </div>

      <div className="px-3 py-1.5 border-t border-t-border flex items-center justify-between">
        <p className="text-[10px] text-t-muted">
          Today starts at {dayRolloverHour}:00 (Settings)
        </p>
        <Button
          variant="ghost"
          size="sm"
          className="h-6 text-[10px]"
          title="Restore the wiped log display, then return to the live view"
          onClick={handleRestore}
        >
          Restore log display
        </Button>
      </div>

      {menu && (
        <EntryContextMenu
          menu={menu}
          onClose={() => setMenu(null)}
          extraItems={[
            {
              label: "Edit game…",
              action: () => {
                // Target the exact row the menu was opened on — path alone
                // is ambiguous when a clip has multiple history events.
                setEditingKey(menu.rowKey);
                setEditValue(menu.entry.game ?? "");
              },
            },
          ]}
        />
      )}
    </div>
  );
}
