import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  clipThumbnail,
  getDiagnostics,
  hideMainWindow,
  hideOverlay,
  overlayClearTarget,
  overlayDescribe,
  overlayGetContext,
  overlayHistory,
  overlayLabel,
  overlayNeedsLabel,
  overlayRate,
  overlaySetGame,
  overlaySetTarget,
  overlaySort,
  overlayTimerReset,
  overlayTimerToggle,
  setWatchPaused,
  showMainWindowNoactivate,
  startTypeMode,
  stopTypeMode,
  undoLastAction,
  wipeLog,
} from "@/lib/commands";
import { EVENTS } from "@/lib/events";
import { overlayViewport } from "@/lib/overlayViewport";
import { errorMessage } from "@/lib/toast";
import type { CountUpTick, OverlayContext, OverlayHistoryRow } from "@/types";

/** Target a "custom text" entry commits to once the user finishes typing. */
type TypingTarget = "label" | "describe" | "game";

type Menu =
  | "root"
  | "sort"
  | "rate"
  | "label"
  | "describe"
  | "game"
  | "history"
  | "app"
  | "timer"
  | { type: "typing"; target: TypingTarget; remember: boolean };

type Flash = { kind: "success" | "error" | "warn"; text: string } | null;

type OverlayTypeEvent =
  | { kind: "char"; ch: string }
  | { kind: "backspace" }
  | { kind: "enter" }
  | { kind: "esc" };

/** Truncate a long filename in the middle so the extension stays visible. */
function middleTruncate(name: string, max = 42): string {
  if (name.length <= max) return name;
  const keep = max - 1;
  const head = Math.ceil(keep * 0.6);
  const tail = keep - head;
  return `${name.slice(0, head)}…${name.slice(name.length - tail)}`;
}

const STAR_LABELS = ["★", "★★", "★★★", "★★★★", "★★★★★"];

/** Thumbnails visible at once in the root menu's clip strip. */
const STRIP_VISIBLE = 4;

/** One row of a flat (non-history, non-typing) menu, as data — the arrow-key
 *  highlight and Enter activation need the row list to be enumerable, not
 *  buried in JSX branches. */
type RowDef = {
  n: number | string;
  label: React.ReactNode;
  hint?: string;
  disabled?: boolean;
  onSelect: () => void;
};

/** mm:ss, minutes unbounded (hours roll into minutes — 74:05 is fine). */
function formatElapsed(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

/** One numbered/clickable row in the overlay's CS:GO-style menu. */
function MenuRow({
  n,
  label,
  hint,
  disabled,
  selected,
  onSelect,
}: {
  n: number | string;
  label: React.ReactNode;
  hint?: string;
  disabled?: boolean;
  selected?: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onMouseDown={(e) => {
        e.preventDefault();
        if (!disabled) onSelect();
      }}
      className={`w-full flex items-center justify-between gap-3 px-3 py-2 rounded-lg text-left
        disabled:opacity-40 disabled:hover:bg-white/5 border transition-colors ${
          selected
            ? "bg-white/15 border-white/25 hover:bg-white/20"
            : "bg-white/5 hover:bg-white/15 border-white/10"
        }`}
    >
      <span className="flex items-center gap-2.5 min-w-0">
        <span className="shrink-0 inline-flex items-center justify-center h-6 w-6 rounded bg-white/10 text-[13px] font-bold font-mono">
          {n}
        </span>
        <span className="truncate text-[15px] font-medium">{label}</span>
      </span>
      {hint && (
        <span className="shrink-0 text-[11px] font-mono text-white/50 uppercase">{hint}</span>
      )}
    </button>
  );
}

// Placeholder shell for the in-game overlay window. The overlay window is
// `transparent(true)` + `decorations(false)` at the Tauri level, but `body`
// in index.css sets `@apply bg-background` (an opaque theme color) so every
// OTHER window paints a solid backdrop. That rule would otherwise paint this
// window's whole viewport opaque too, so this component overrides background
// to transparent on its own root (inline styles, not a CSS file edit — keeps
// the override scoped to this window only).
export function OverlayApp() {
  useEffect(() => {
    // index.css's `body { @apply bg-background }` paints an opaque color
    // that fills the whole (Tauri-transparent) window surface — override it
    // here rather than editing the shared stylesheet, so no other window is
    // affected.
    // The boot <style> in index.html also paints #root opaque — override
    // all three surfaces the boot script targets.
    const root = document.getElementById("root");
    const prevBodyBg = document.body.style.background;
    const prevHtmlBg = document.documentElement.style.background;
    const prevRootBg = root?.style.background ?? "";
    document.body.style.background = "transparent";
    document.documentElement.style.background = "transparent";
    if (root) root.style.background = "transparent";
    return () => {
      document.body.style.background = prevBodyBg;
      document.documentElement.style.background = prevHtmlBg;
      if (root) root.style.background = prevRootBg;
    };
  }, []);

  const [ctx, setCtx] = useState<OverlayContext | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [menu, setMenu] = useState<Menu>("root");
  const [typingBuffer, setTypingBuffer] = useState("");
  const [flash, setFlash] = useState<Flash>(null);
  // Watcher-pause state for the App submenu's row-1 label — fetched fresh
  // every time the submenu is entered (Diagnostics isn't otherwise pushed
  // to this window).
  const [watchPaused, setWatchPausedState] = useState<boolean | null>(null);
  const watchPausedRef = useRef<boolean | null>(null);
  useEffect(() => {
    watchPausedRef.current = watchPaused;
  }, [watchPaused]);
  // Live count-up stopwatch — same event the main window's TimerDisplay
  // consumes. Drives both the header "⏱ mm:ss" readout and the
  // App/Timer submenu row labels (Start vs Stop).
  const [countUp, setCountUp] = useState<CountUpTick>({ elapsedSecs: 0, running: false });
  const countUpRef = useRef<CountUpTick>(countUp);
  useEffect(() => {
    countUpRef.current = countUp;
  }, [countUp]);

  // History rolodex state ("Today's clips") — rows/sel/off mirrored into refs
  // for the overlay-key listener, same pattern as the other menus.
  const [historyRows, setHistoryRows] = useState<OverlayHistoryRow[]>([]);
  const [historySel, setHistorySel] = useState(0);
  const [historyOff, setHistoryOff] = useState(0);
  const historyRowsRef = useRef<OverlayHistoryRow[]>([]);
  const historySelRef = useRef(0);
  const historyOffRef = useRef(0);
  useEffect(() => {
    historyRowsRef.current = historyRows;
  }, [historyRows]);
  useEffect(() => {
    historySelRef.current = historySel;
  }, [historySel]);
  useEffect(() => {
    historyOffRef.current = historyOff;
  }, [historyOff]);
  // Thumbnail strip (root menu): its own selection/offset over the SAME
  // historyRows, independent of the history submenu's rolodex selection.
  // Index 0 = latest clip (rows are newest-first).
  const [stripSel, setStripSel] = useState(0);
  const [stripOff, setStripOff] = useState(0);
  const stripSelRef = useRef(0);
  const stripOffRef = useRef(0);
  // path → data URL; a `null` value means requested-but-failed (placeholder).
  // Failed entries are dropped on every overlay open so a clip whose shell
  // thumbnail wasn't ready yet (freshly recorded) gets retried.
  const thumbsRef = useRef(new Map<string, string | null>());
  const [, setThumbTick] = useState(0);
  // Arrow-key row highlight for the flat menus (root + submenus; the history
  // rolodex and typing mode have their own selection models).
  const [rowSel, setRowSel] = useState(0);
  const rowSelRef = useRef(0);
  const rowDefsRef = useRef<RowDef[]>([]);
  // Tracks whether the backend's acting-clip target was set as of the last
  // fetchContext() — lets fetchContext detect the target vanishing out from
  // under us (fromHistory flips true -> false without us clearing it).
  const wasTargetedRef = useRef(false);
  useEffect(() => {
    const un = listen<CountUpTick>(EVENTS.COUNT_UP_TICK, (e) => {
      setCountUp(e.payload);
    });
    return () => {
      un.then((fn) => fn());
    };
  }, []);

  // Refs mirror state that the overlay-key / overlay-type listeners need to
  // read synchronously — the listeners are registered once and must always
  // see the CURRENT menu/buffer, not whatever was captured at mount time.
  const menuRef = useRef<Menu>(menu);
  const bufferRef = useRef("");
  const ctxRef = useRef<OverlayContext | null>(ctx);
  // Mirrors `flash` for the hotkey listener: while a success/error flash is
  // pending, digit input must be ignored — the flash swaps the JSX so mouse
  // can't re-fire, but global hotkeys bypass the DOM entirely (double-tapping
  // a digit would double-sort and keep extending the close timer).
  const flashRef = useRef<Flash>(null);
  useEffect(() => {
    flashRef.current = flash;
  }, [flash]);
  useEffect(() => {
    menuRef.current = menu;
  }, [menu]);
  useEffect(() => {
    ctxRef.current = ctx;
  }, [ctx]);

  const flashTimer = useRef<number | null>(null);
  const showFlash = useCallback((kind: "success" | "error" | "warn", text: string) => {
    // Sync the ref immediately — the next overlay-key event can arrive before
    // React flushes the state effect, and it must already see the flash.
    flashRef.current = { kind, text };
    setFlash({ kind, text });
  }, []);

  const closeAfter = useCallback((ms: number) => {
    if (flashTimer.current) window.clearTimeout(flashTimer.current);
    flashTimer.current = window.setTimeout(() => {
      hideOverlay().catch(() => {});
    }, ms);
  }, []);

  const fetchContext = useCallback(async () => {
    try {
      const c = await overlayGetContext();
      // Flag the target vanishing out from under us — the backend dropped
      // fromHistory without an explicit overlayClearTarget() call from here
      // (e.g. the targeted clip's file was deleted). Deliberate clears set
      // wasTargetedRef to false themselves before calling fetchContext, so
      // this only fires for the unexpected case.
      if (wasTargetedRef.current && !c.fromHistory) {
        flashRef.current = { kind: "warn", text: "Clip no longer exists — back to latest" };
        setFlash(flashRef.current);
        if (flashTimer.current) window.clearTimeout(flashTimer.current);
        flashTimer.current = window.setTimeout(() => {
          flashRef.current = null;
          setFlash(null);
        }, 1500);
      }
      wasTargetedRef.current = c.fromHistory;
      setCtx(c);
      setLoadError(null);
    } catch (e) {
      setCtx(null);
      setLoadError(errorMessage(e));
    }
  }, []);

  const resetToRoot = useCallback(() => {
    setMenu("root");
    setTypingBuffer("");
    bufferRef.current = "";
    // Strip back to the latest clip (the backend target was cleared on hide).
    stripSelRef.current = 0;
    stripOffRef.current = 0;
    setStripSel(0);
    setStripOff(0);
    // Drop failed thumbnail fetches so they retry — a clip recorded seconds
    // before the last open often has no shell thumbnail yet.
    for (const [k, v] of thumbsRef.current) {
      if (v === null) thumbsRef.current.delete(k);
    }
    // Every overlay close clears the backend's acting-clip target by design
    // (overlay.rs hide()). Acknowledge that here so the next reopen's
    // fetchContext doesn't mistake the routine reset for the targeted clip
    // vanishing and flash a false "Clip no longer exists" warning.
    wasTargetedRef.current = false;
    flashRef.current = null;
    setFlash(null);
    if (flashTimer.current) {
      window.clearTimeout(flashTimer.current);
      flashTimer.current = null;
    }
    stopTypeMode().catch(() => {});
  }, []);

  // Refetch today's rows for the thumbnail strip, clamping its selection
  // against the (possibly shorter) refreshed list.
  const refreshStrip = useCallback(() => {
    overlayHistory()
      .then((rows) => {
        setHistoryRows(rows);
        const sel = Math.max(0, Math.min(stripSelRef.current, rows.length - 1));
        const vp = overlayViewport(rows.length, sel, stripOffRef.current, STRIP_VISIBLE);
        stripSelRef.current = sel;
        stripOffRef.current = vp.offset;
        setStripSel(sel);
        setStripOff(vp.offset);
      })
      .catch(() => {});
  }, []);
  const refreshStripRef = useRef(refreshStrip);
  refreshStripRef.current = refreshStrip;

  // Initial fetch + refetch every time the main app re-opens the overlay.
  useEffect(() => {
    fetchContext();
    refreshStripRef.current();
    const unOpen = listen("overlay-open", () => {
      resetToRoot();
      fetchContext();
      refreshStripRef.current();
    });
    const unVisible = listen<{ visible: boolean }>("overlay-visible", (e) => {
      if (!e.payload?.visible) resetToRoot();
    });
    return () => {
      unOpen.then((fn) => fn());
      unVisible.then((fn) => fn());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Always stop type mode when this window's UI tears down — the LL keyboard
  // hook must never keep swallowing the game's keystrokes after the overlay
  // component is gone, no matter how we got here.
  useEffect(() => {
    return () => {
      stopTypeMode().catch(() => {});
    };
  }, []);

  const runAction = useCallback(
    async (fn: () => Promise<void>, successText: string) => {
      try {
        await fn();
        showFlash("success", successText);
        closeAfter(1000);
      } catch (e) {
        showFlash("error", errorMessage(e));
        closeAfter(1500);
      }
    },
    [showFlash, closeAfter],
  );

  // Clears the flash without hiding the overlay — for actions (Undo, App
  // submenu, Timer submenu) that must leave the overlay open afterward.
  const flashOnly = useCallback((ms: number) => {
    if (flashTimer.current) window.clearTimeout(flashTimer.current);
    flashTimer.current = window.setTimeout(() => {
      flashRef.current = null;
      setFlash(null);
    }, ms);
  }, []);

  // Same shape as `runAction` but keeps the overlay open — used by actions
  // that make sense to fire repeatedly without re-summoning the overlay
  // (Undo, watch pause/resume, count-up start/stop, etc).
  const runInPlace = useCallback(
    async (fn: () => Promise<void>, successText: string) => {
      try {
        await fn();
        showFlash("success", successText);
        flashOnly(900);
      } catch (e) {
        showFlash("error", errorMessage(e));
        flashOnly(1500);
      }
    },
    [showFlash, flashOnly],
  );

  // Sets the backend's acting-clip target to a history row and returns to
  // root. On failure, stays in the history menu and refetches the list —
  // the failure (e.g. the file vanished between fetch and pick) means the
  // row's `exists` flag is stale.
  const pickHistoryRow = useCallback(
    async (row: OverlayHistoryRow) => {
      if (flashRef.current) return;
      try {
        await overlaySetTarget(row.path);
        await fetchContext();
        // Mirror the pick into the thumbnail strip so root shows the same
        // clip selected.
        const idx = historyRowsRef.current.findIndex((r) => r.path === row.path);
        if (idx >= 0) {
          const vp = overlayViewport(
            historyRowsRef.current.length,
            idx,
            stripOffRef.current,
            STRIP_VISIBLE,
          );
          stripSelRef.current = idx;
          stripOffRef.current = vp.offset;
          setStripSel(idx);
          setStripOff(vp.offset);
        }
        setMenu("root");
      } catch (e) {
        showFlash("error", errorMessage(e));
        flashOnly(1500);
        overlayHistory()
          .then((rows) => {
            // Re-clamp selection/offset against the refreshed (possibly
            // shorter) list, or the visible slice can render empty until an
            // arrow press forces a recompute.
            const sel = Math.max(0, Math.min(historySelRef.current, rows.length - 1));
            const { offset } = overlayViewport(rows.length, sel, historyOffRef.current);
            setHistoryRows(rows);
            setHistorySel(sel);
            setHistoryOff(offset);
          })
          .catch(() => {});
      }
    },
    [fetchContext, showFlash, flashOnly],
  );

  // Clears the backend's acting-clip target, reverting to the latest clip.
  // Marks wasTargetedRef false first so fetchContext doesn't mistake this
  // deliberate clear for the target vanishing out from under us.
  const backToLatest = useCallback(async () => {
    if (flashRef.current) return;
    wasTargetedRef.current = false;
    stripSelRef.current = 0;
    stripOffRef.current = 0;
    setStripSel(0);
    setStripOff(0);
    try {
      await overlayClearTarget();
      await fetchContext();
      showFlash("success", "Back to latest clip");
      flashOnly(900);
    } catch (e) {
      showFlash("error", errorMessage(e));
      flashOnly(1500);
    }
  }, [fetchContext, showFlash, flashOnly]);

  // Move the thumbnail strip's selection to `target` and retarget the
  // backend's acting clip to match (index 0 = back to the latest clip).
  // Vanished clips can be browsed past but are never targeted.
  const stripSeek = useCallback(
    (target: number) => {
      if (flashRef.current) return;
      const rows = historyRowsRef.current;
      if (rows.length === 0) return;
      const sel = Math.max(0, Math.min(rows.length - 1, target));
      if (sel === stripSelRef.current) return;
      const vp = overlayViewport(rows.length, sel, stripOffRef.current, STRIP_VISIBLE);
      stripSelRef.current = sel;
      stripOffRef.current = vp.offset;
      setStripSel(sel);
      setStripOff(vp.offset);
      const row = rows[sel];
      if (sel !== 0 && !row.exists) return;
      // Deliberate retarget — must not read as "target vanished" (index 0
      // flips fromHistory back to false, which the vanish heuristic watches).
      wasTargetedRef.current = false;
      const act = sel === 0 ? overlayClearTarget() : overlaySetTarget(row.path);
      act.then(fetchContext).catch((e) => {
        showFlash("error", errorMessage(e));
        flashOnly(1200);
      });
    },
    [fetchContext, showFlash, flashOnly],
  );

  // Fetch shell thumbnails for the strip's visible slice (backend caches;
  // the Map here just avoids re-invoking per render).
  useEffect(() => {
    const visible = historyRows.slice(stripOff, stripOff + STRIP_VISIBLE);
    for (const row of visible) {
      if (!row.exists || thumbsRef.current.has(row.path)) continue;
      thumbsRef.current.set(row.path, null);
      clipThumbnail(row.path)
        .then((url) => {
          thumbsRef.current.set(row.path, url);
          setThumbTick((t) => t + 1);
        })
        .catch(() => {
          // Stays null → placeholder; retried on the next overlay open.
          setThumbTick((t) => t + 1);
        });
    }
  }, [historyRows, stripOff]);

  const needsTypingFallback = useCallback(async () => {
    try {
      await overlayNeedsLabel();
    } catch {
      // best-effort log nudge; nothing else to do if it fails too
    }
    showFlash("warn", "Typing is disabled in Settings — reminder logged.");
    closeAfter(1500);
  }, [showFlash, closeAfter]);

  const enterTyping = useCallback(
    async (target: TypingTarget, remember = false) => {
      if (!ctxRef.current?.typingEnabled) {
        await needsTypingFallback();
        return;
      }
      try {
        await startTypeMode();
        setTypingBuffer("");
        bufferRef.current = "";
        setMenu({ type: "typing", target, remember });
      } catch {
        await needsTypingFallback();
      }
    },
    [needsTypingFallback],
  );

  const submenuFor = (target: TypingTarget): Menu =>
    target === "label" ? "label" : target === "describe" ? "describe" : "game";

  const cancelTyping = useCallback((target: TypingTarget) => {
    stopTypeMode().catch(() => {});
    setTypingBuffer("");
    bufferRef.current = "";
    setMenu(submenuFor(target));
  }, []);

  const commitTyping = useCallback(
    async (target: TypingTarget, remember: boolean) => {
      const text = bufferRef.current.trim();
      stopTypeMode().catch(() => {});
      if (!text) {
        setMenu(submenuFor(target));
        return;
      }
      if (target === "label") {
        await runAction(() => overlayLabel(text), `Labeled: ${text}`);
      } else if (target === "describe") {
        await runAction(() => overlayDescribe(text), "Description saved");
      } else {
        await runAction(() => overlaySetGame(text, remember), `Game set: ${text}`);
      }
    },
    [runAction],
  );

  // Feed typing-mode keystrokes from the LL keyboard hook. Registered once;
  // reads the current menu/buffer through refs so it never goes stale.
  useEffect(() => {
    const un = listen<OverlayTypeEvent>("overlay-type", (e) => {
      const m = menuRef.current;
      if (typeof m !== "object" || m.type !== "typing") return;
      const payload = e.payload;
      if (payload.kind === "char") {
        bufferRef.current += payload.ch;
        setTypingBuffer(bufferRef.current);
      } else if (payload.kind === "backspace") {
        bufferRef.current = bufferRef.current.slice(0, -1);
        setTypingBuffer(bufferRef.current);
      } else if (payload.kind === "enter") {
        commitTyping(m.target, m.remember);
      } else if (payload.kind === "esc") {
        cancelTyping(m.target);
      }
    });
    return () => {
      un.then((fn) => fn());
    };
  }, [commitTyping, cancelTyping]);

  // Digit dispatch shared by the overlay-key hotkey listener and mouse clicks.
  const selectDigit = useCallback(
    (n: number) => {
      const m = menuRef.current;
      const c = ctxRef.current;
      if (typeof m === "object") return; // typing mode ignores digits
      if (flashRef.current) return; // action pending/flashing — no re-fire

      // Navigation keys: 11 = Up, 12 = Down, 13 = Left, 14 = Right,
      // 15 = Enter (W/S/A/D alias the arrows when the Settings toggle is on).
      if (n >= 11 && n <= 15) {
        if (m === "history") {
          // The rolodex keeps its own vertical selection; Enter targets it.
          if (n === 11 || n === 12) {
            const rows = historyRowsRef.current;
            if (rows.length === 0) return;
            const sel =
              n === 11
                ? Math.max(0, historySelRef.current - 1)
                : Math.min(rows.length - 1, historySelRef.current + 1);
            const vp = overlayViewport(rows.length, sel, historyOffRef.current);
            historySelRef.current = sel;
            historyOffRef.current = vp.offset;
            setHistorySel(sel);
            setHistoryOff(vp.offset);
          } else if (n === 15) {
            const row = historyRowsRef.current[historySelRef.current];
            if (row && row.exists) pickHistoryRow(row);
          }
          return;
        }
        if (n === 11 || n === 12) {
          // Move the row highlight, skipping disabled rows.
          const defs = rowDefsRef.current;
          if (defs.length === 0) return;
          const dir = n === 11 ? -1 : 1;
          let i = rowSelRef.current + dir;
          while (i >= 0 && i < defs.length && defs[i].disabled) i += dir;
          if (i < 0 || i >= defs.length) return;
          rowSelRef.current = i;
          setRowSel(i);
          return;
        }
        if (n === 13 || n === 14) {
          // Left/Right drive the root menu's thumbnail strip: left = newer,
          // right = older (rows are newest-first).
          if (m !== "root") return;
          stripSeek(stripSelRef.current + (n === 13 ? -1 : 1));
          return;
        }
        // Enter — activate the highlighted row.
        const d = rowDefsRef.current[rowSelRef.current];
        if (d && !d.disabled) d.onSelect();
        return;
      }

      if (m === "root") {
        if (n === 1) setMenu("sort");
        else if (n === 2) setMenu("rate");
        else if (n === 3) setMenu("label");
        else if (n === 4) setMenu("describe");
        else if (n === 5) setMenu("game");
        else if (n === 6) setMenu("timer");
        else if (n === 7) {
          setMenu("history");
          historySelRef.current = 0;
          historyOffRef.current = 0;
          setHistorySel(0);
          setHistoryOff(0);
          overlayHistory()
            .then(setHistoryRows)
            .catch(() => setHistoryRows([]));
        } else if (n === 8) {
          runInPlace(async () => {
            await undoLastAction();
            await fetchContext();
            // Undo can rename/restore files — the strip's rows are stale.
            refreshStripRef.current();
          }, "Undid last action");
        } else if (n === 9) {
          setMenu("app");
          getDiagnostics()
            .then((d) => {
              watchPausedRef.current = d.watchPaused;
              setWatchPausedState(d.watchPaused);
            })
            .catch(() => {});
        }
        return;
      }

      if (m === "sort") {
        if (n === 0) {
          setMenu("root");
        } else if (n >= 1 && n <= 3) {
          const name =
            n === 1 ? c?.binds.g1Name : n === 2 ? c?.binds.g2Name : c?.binds.g3Name;
          runAction(() => overlaySort(n), `Sorted → ${name ?? `G${n}`}`);
        }
        return;
      }

      if (m === "rate") {
        if (n === 0) setMenu("root");
        else if (n >= 1 && n <= 5)
          runAction(() => overlayRate(n), `Rated ${STAR_LABELS[n - 1]}`);
        return;
      }

      if (m === "label") {
        if (n === 0) {
          enterTyping("label");
        } else {
          const preset = c?.labelPresets[n - 1];
          if (preset) runAction(() => overlayLabel(preset), `Labeled: ${preset}`);
        }
        return;
      }

      if (m === "describe") {
        if (n === 0) {
          enterTyping("describe");
        } else {
          const preset = c?.descriptionPresets[n - 1];
          if (preset) runAction(() => overlayDescribe(preset), "Description saved");
        }
        return;
      }

      if (m === "game") {
        if (n === 0) setMenu("root");
        else if (n === 1) {
          hideOverlay().catch(() => {});
        } else if (n === 2) {
          enterTyping("game", false);
        } else if (n === 3 && c?.exe) {
          enterTyping("game", true);
        }
        return;
      }

      if (m === "history") {
        if (n === 0) {
          setMenu("root");
        } else if (n >= 1 && n <= 7) {
          const idx = historyOffRef.current + (n - 1);
          const row = historyRowsRef.current[idx];
          if (row && row.exists) pickHistoryRow(row);
        }
        return;
      }

      if (m === "app") {
        if (n === 0) setMenu("root");
        else if (n === 1) {
          // Guard the pre-fetch window: until getDiagnostics resolves we
          // don't know the real pause state, and guessing can send the
          // toggle the wrong direction. Ignore the press instead.
          const paused = watchPausedRef.current;
          if (paused === null) return;
          runInPlace(async () => {
            await setWatchPaused(!paused);
            watchPausedRef.current = !paused;
            setWatchPausedState(!paused);
          }, paused ? "Watching resumed" : "Watching paused");
        } else if (n === 2) {
          runInPlace(() => showMainWindowNoactivate(), "ClipShelf window shown");
        } else if (n === 3) {
          runInPlace(() => hideMainWindow(), "Hidden to tray");
        } else if (n === 4) {
          runInPlace(() => wipeLog(), "Wiped current clip");
        } else if (n === 5) {
          const running = countUpRef.current.running;
          runInPlace(() => overlayTimerToggle(), running ? "Count-up stopped" : "Count-up started");
        }
        return;
      }

      if (m === "timer") {
        if (n === 0) setMenu("root");
        else if (n === 1) {
          const running = countUpRef.current.running;
          runInPlace(() => overlayTimerToggle(), running ? "Count-up stopped" : "Count-up started");
        } else if (n === 2) {
          runInPlace(() => overlayTimerReset(), "Stopwatch reset");
        }
      }
    },
    [runAction, enterTyping, runInPlace, fetchContext, pickHistoryRow, stripSeek],
  );

  useEffect(() => {
    const un = listen<number>("overlay-key", (e) => {
      selectDigit(e.payload);
    });
    return () => {
      un.then((fn) => fn());
    };
  }, [selectDigit]);

  useEffect(() => {
    return () => {
      if (flashTimer.current) window.clearTimeout(flashTimer.current);
    };
  }, []);

  // Recomputed every render (cheap — at most 30 rows) rather than mirrored
  // into state, so it always reflects the latest historyRows/sel/off.
  const historyVp = overlayViewport(historyRows.length, historySel, historyOff);
  const historyVisibleRows = historyRows.slice(
    historyVp.offset,
    historyVp.offset + 7,
  );
  const stripVp = overlayViewport(historyRows.length, stripSel, stripOff, STRIP_VISIBLE);
  const stripRows = historyRows.slice(stripVp.offset, stripVp.offset + STRIP_VISIBLE);

  // The flat menus as data — drives rendering AND the arrow/Enter nav.
  const rowDefs: RowDef[] = (() => {
    if (!ctx || typeof menu !== "string") return [];
    switch (menu) {
      case "root": {
        const defs: RowDef[] = [
          { n: 1, label: "Sort", hint: "G1/G2/G3", onSelect: () => selectDigit(1) },
          { n: 2, label: "Rate", onSelect: () => selectDigit(2) },
          { n: 3, label: "Label", onSelect: () => selectDigit(3) },
          { n: 4, label: "Description", onSelect: () => selectDigit(4) },
          { n: 5, label: "Game", onSelect: () => selectDigit(5) },
          { n: 6, label: "Timer", onSelect: () => selectDigit(6) },
          { n: 7, label: "History", onSelect: () => selectDigit(7) },
          { n: 8, label: "Undo", onSelect: () => selectDigit(8) },
          { n: 9, label: "App", onSelect: () => selectDigit(9) },
        ];
        if (ctx.fromHistory) {
          defs.push({ n: "L", label: "Back to latest clip", onSelect: backToLatest });
        }
        return defs;
      }
      case "sort":
        return [
          { n: 1, label: ctx.binds.g1Name || "G1", hint: ctx.binds.g1, onSelect: () => selectDigit(1) },
          { n: 2, label: ctx.binds.g2Name || "G2", hint: ctx.binds.g2, onSelect: () => selectDigit(2) },
          { n: 3, label: ctx.binds.g3Name || "G3", hint: ctx.binds.g3, onSelect: () => selectDigit(3) },
          { n: 0, label: "Back", onSelect: () => selectDigit(0) },
        ];
      case "rate":
        return [
          ...STAR_LABELS.map((stars, i) => ({
            n: i + 1,
            label: <span className="text-amber-400">{stars}</span>,
            onSelect: () => selectDigit(i + 1),
          })),
          { n: 0, label: "Back", onSelect: () => selectDigit(0) },
        ];
      case "label":
        return [
          ...ctx.labelPresets.slice(0, 9).map((p, i) => ({
            n: i + 1,
            label: p,
            onSelect: () => selectDigit(i + 1),
          })),
          { n: 0, label: "Custom…", onSelect: () => selectDigit(0) },
        ];
      case "describe":
        return [
          ...ctx.descriptionPresets.slice(0, 9).map((p, i) => ({
            n: i + 1,
            label: p,
            onSelect: () => selectDigit(i + 1),
          })),
          { n: 0, label: "Custom…", onSelect: () => selectDigit(0) },
        ];
      case "game":
        return [
          {
            n: 1,
            label: ctx.game ? `Keep "${ctx.game}"` : "Keep (no game detected)",
            onSelect: () => selectDigit(1),
          },
          { n: 2, label: "Edit (type)", onSelect: () => selectDigit(2) },
          { n: 3, label: "Edit & remember", disabled: !ctx.exe, onSelect: () => selectDigit(3) },
          { n: 0, label: "Back", onSelect: () => selectDigit(0) },
        ];
      case "app":
        return [
          {
            n: 1,
            label:
              watchPaused === null
                ? "Loading watcher state…"
                : watchPaused
                  ? "Resume watching"
                  : "Pause watching",
            disabled: watchPaused === null,
            onSelect: () => selectDigit(1),
          },
          { n: 2, label: "Open ClipShelf window", onSelect: () => selectDigit(2) },
          { n: 3, label: "Hide to tray", onSelect: () => selectDigit(3) },
          { n: 4, label: "Wipe current clip", onSelect: () => selectDigit(4) },
          {
            n: 5,
            label: countUp.running ? "Stop count-up" : "Start count-up",
            onSelect: () => selectDigit(5),
          },
          { n: 0, label: "Back", onSelect: () => selectDigit(0) },
        ];
      case "timer":
        return [
          {
            n: 1,
            label: countUp.running ? "Stop count-up" : "Start count-up",
            onSelect: () => selectDigit(1),
          },
          { n: 2, label: "Reset", onSelect: () => selectDigit(2) },
          { n: 0, label: "Back", onSelect: () => selectDigit(0) },
        ];
      default:
        return [];
    }
  })();
  rowDefsRef.current = rowDefs;

  // Reset the row highlight to the first enabled row whenever the menu
  // changes (runs after render, so it sees the fresh rowDefs).
  const menuKey = typeof menu === "string" ? menu : "typing";
  useEffect(() => {
    const first = rowDefsRef.current.findIndex((d) => !d.disabled);
    const sel = first < 0 ? 0 : first;
    rowSelRef.current = sel;
    setRowSel(sel);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [menuKey]);

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "transparent",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <div className="bg-black/85 rounded-2xl border border-white/10 text-white shadow-2xl w-[380px] max-h-[440px] flex flex-col overflow-hidden">
        {!ctx && !loadError && (
          <div className="p-6 text-center text-white/60 text-sm">Loading…</div>
        )}

        {!ctx && loadError && (
          <div className="p-6 text-center space-y-1">
            <p className="text-[15px] font-semibold text-white/80">
              {loadError || "No recent clip"}
            </p>
            <p className="text-[11px] text-white/40">
              Save a clip first, then reopen the overlay.
            </p>
          </div>
        )}

        {ctx && (
          <>
            {/* Header */}
            <div className="px-4 pt-4 pb-3 border-b border-white/10 space-y-1.5">
              <div className="flex items-center justify-between gap-2">
                <span className="text-[15px] font-semibold truncate" title={ctx.filename}>
                  {ctx.fromHistory && <span className="text-primary mr-1">▸</span>}
                  {middleTruncate(ctx.filename)}
                </span>
                {ctx.binds.overlay && (
                  <span className="shrink-0 text-[10px] font-mono px-1.5 py-0.5 rounded bg-white/10 text-white/50">
                    {ctx.binds.overlay}
                  </span>
                )}
              </div>
              <div className="flex items-center gap-2">
                {ctx.game && (
                  <span className="inline-flex items-center px-2 py-0.5 rounded-full bg-primary/20 text-primary text-[11px] font-medium">
                    {ctx.game}
                  </span>
                )}
                {countUp.running && (
                  <span className="text-[11px] font-mono text-white/70">
                    ⏱ {formatElapsed(countUp.elapsedSecs)}
                  </span>
                )}
              </div>
              {ctx.fromHistory && (
                <div className="text-[11px] text-white/40">
                  from history{ctx.game ? ` · ${ctx.game}` : ""}
                  {ctx.targetTime ? ` · ${ctx.targetTime}` : ""}
                </div>
              )}
            </div>

            {/* Thumbnail strip — today's clips, latest first; ◀/▶ (or A/D)
                moves the selection and retargets every menu action to the
                selected clip. */}
            {menu === "root" && historyRows.length > 0 && (
              <div className="px-3 py-2.5 border-b border-white/10">
                <div className="flex items-center gap-1">
                  <span
                    className={`shrink-0 w-3 text-center text-[10px] ${
                      stripVp.dotsAbove ? "text-white/60" : "text-white/15"
                    }`}
                  >
                    ◀
                  </span>
                  <div className="flex-1 flex justify-center gap-1.5 min-w-0">
                    {stripRows.map((row, i) => {
                      const idx = stripVp.offset + i;
                      const isSel = idx === stripSel;
                      const url = thumbsRef.current.get(row.path);
                      return (
                        <button
                          key={row.path}
                          type="button"
                          onMouseDown={(e) => {
                            e.preventDefault();
                            stripSeek(idx);
                          }}
                          className="shrink-0 w-[74px]"
                        >
                          <div
                            className={`aspect-video rounded-md overflow-hidden border transition-colors ${
                              isSel
                                ? "border-primary ring-1 ring-primary"
                                : "border-white/10 hover:border-white/30"
                            } ${row.exists ? "" : "opacity-40"}`}
                          >
                            {url ? (
                              <img
                                src={url}
                                alt=""
                                draggable={false}
                                className="w-full h-full object-cover"
                              />
                            ) : (
                              <div className="w-full h-full bg-white/5 flex items-center justify-center text-white/30 text-sm">
                                🎬
                              </div>
                            )}
                          </div>
                          <div
                            className={`mt-0.5 text-[9px] truncate text-center ${
                              isSel ? "text-white/80" : "text-white/40"
                            }`}
                          >
                            {idx === 0 ? "latest" : row.time}
                          </div>
                        </button>
                      );
                    })}
                  </div>
                  <span
                    className={`shrink-0 w-3 text-center text-[10px] ${
                      stripVp.dotsBelow ? "text-white/60" : "text-white/15"
                    }`}
                  >
                    ▶
                  </span>
                </div>
              </div>
            )}

            {/* Body */}
            <div className="flex-1 overflow-y-auto px-3 py-3 space-y-1.5">
              {flash ? (
                <div
                  className={`px-3 py-4 text-center text-[15px] font-medium rounded-lg ${
                    flash.kind === "success"
                      ? "text-green-400"
                      : flash.kind === "warn"
                        ? "text-amber-400"
                        : "text-red-400"
                  }`}
                >
                  {flash.text}
                </div>
              ) : typeof menu === "string" && menu !== "history" ? (
                <>
                  {rowDefs.map((r, i) => (
                    <MenuRow
                      key={i}
                      n={r.n}
                      label={r.label}
                      hint={r.hint}
                      disabled={r.disabled}
                      selected={i === rowSel}
                      onSelect={r.onSelect}
                    />
                  ))}
                </>
              ) : menu === "history" ? (
                <>
                  <div className="px-1 pb-1 text-[11px] uppercase tracking-wide text-white/40">
                    Today&rsquo;s clips
                  </div>
                  {historyVp.dotsAbove && (
                    <div className="text-center text-[11px] text-white/40">▲ more</div>
                  )}
                  {historyRows.length === 0 ? (
                    <div className="px-3 py-6 text-center text-white/40 text-[13px]">
                      No clips yet today
                    </div>
                  ) : (
                    historyVisibleRows.map((row, i) => (
                      <MenuRow
                        key={row.path}
                        n={i + 1}
                        label={middleTruncate(row.filename)}
                        hint={row.game ?? row.time}
                        disabled={!row.exists}
                        selected={historyVp.offset + i === historySel}
                        onSelect={() => pickHistoryRow(row)}
                      />
                    ))
                  )}
                  {historyVp.dotsBelow && (
                    <div className="text-center text-[11px] text-white/40">▼ more</div>
                  )}
                  <MenuRow n={0} label="Back" onSelect={() => selectDigit(0)} />
                </>
              ) : (
                // Typing mode
                <div className="space-y-3 py-2">
                  <p className="text-[11px] uppercase tracking-wide text-white/40">
                    {menu.target === "label"
                      ? "Label"
                      : menu.target === "describe"
                        ? "Description"
                        : menu.target === "game" && menu.remember
                          ? "Game (remember)"
                          : "Game"}
                  </p>
                  <div className="px-3 py-2.5 rounded-lg bg-white/5 border border-white/10 font-mono text-[15px] min-h-[2.5rem] flex items-center">
                    <span className="whitespace-pre-wrap break-all">{typingBuffer}</span>
                    <span className="inline-block w-[2px] h-[1.1em] bg-white ml-0.5 animate-pulse" />
                  </div>
                </div>
              )}
            </div>

            {/* Footer */}
            <div className="px-4 py-2 border-t border-white/10 text-[10px] text-white/40 text-center">
              {typeof menu === "object"
                ? "Enter confirms · Esc cancels"
                : menu === "root"
                  ? "Esc closes · ◀ ▶ pick clip · ▲ ▼ + Enter or number"
                  : "Esc closes · ▲ ▼ + Enter or number"}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
