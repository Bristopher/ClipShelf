import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  getDiagnostics,
  hideMainWindow,
  hideOverlay,
  overlayDescribe,
  overlayGetContext,
  overlayLabel,
  overlayNeedsLabel,
  overlayRate,
  overlaySetGame,
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
import { errorMessage } from "@/lib/toast";
import type { CountUpTick, OverlayContext } from "@/types";

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
  onSelect,
}: {
  n: number | string;
  label: React.ReactNode;
  hint?: string;
  disabled?: boolean;
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
      className="w-full flex items-center justify-between gap-3 px-3 py-2 rounded-lg text-left
        bg-white/5 hover:bg-white/15 disabled:opacity-40 disabled:hover:bg-white/5
        border border-white/10 transition-colors"
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
    flashRef.current = null;
    setFlash(null);
    if (flashTimer.current) {
      window.clearTimeout(flashTimer.current);
      flashTimer.current = null;
    }
    stopTypeMode().catch(() => {});
  }, []);

  // Initial fetch + refetch every time the main app re-opens the overlay.
  useEffect(() => {
    fetchContext();
    const unOpen = listen("overlay-open", () => {
      resetToRoot();
      fetchContext();
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

      if (m === "root") {
        if (n === 1) setMenu("sort");
        else if (n === 2) setMenu("rate");
        else if (n === 3) setMenu("label");
        else if (n === 4) setMenu("describe");
        else if (n === 5) setMenu("game");
        else if (n === 6) setMenu("timer");
        else if (n === 7) setMenu("history");
        else if (n === 8) {
          runInPlace(async () => {
            await undoLastAction();
            await fetchContext();
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
        if (n === 0) setMenu("root");
        // Task 6 wires the list itself; this shell just handles Back.
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
    [runAction, enterTyping, runInPlace, fetchContext],
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
            </div>

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
              ) : menu === "root" ? (
                <>
                  <MenuRow n={1} label="Sort" hint="G1/G2/G3" onSelect={() => selectDigit(1)} />
                  <MenuRow n={2} label="Rate" onSelect={() => selectDigit(2)} />
                  <MenuRow n={3} label="Label" onSelect={() => selectDigit(3)} />
                  <MenuRow n={4} label="Description" onSelect={() => selectDigit(4)} />
                  <MenuRow n={5} label="Game" onSelect={() => selectDigit(5)} />
                  <MenuRow n={6} label="Timer" onSelect={() => selectDigit(6)} />
                  <MenuRow n={7} label="History" onSelect={() => selectDigit(7)} />
                  <MenuRow n={8} label="Undo" onSelect={() => selectDigit(8)} />
                  <MenuRow n={9} label="App" onSelect={() => selectDigit(9)} />
                </>
              ) : menu === "sort" ? (
                <>
                  <MenuRow
                    n={1}
                    label={ctx.binds.g1Name || "G1"}
                    hint={ctx.binds.g1}
                    onSelect={() => selectDigit(1)}
                  />
                  <MenuRow
                    n={2}
                    label={ctx.binds.g2Name || "G2"}
                    hint={ctx.binds.g2}
                    onSelect={() => selectDigit(2)}
                  />
                  <MenuRow
                    n={3}
                    label={ctx.binds.g3Name || "G3"}
                    hint={ctx.binds.g3}
                    onSelect={() => selectDigit(3)}
                  />
                  <MenuRow n={0} label="Back" onSelect={() => selectDigit(0)} />
                </>
              ) : menu === "rate" ? (
                <>
                  {STAR_LABELS.map((stars, i) => (
                    <MenuRow
                      key={i}
                      n={i + 1}
                      label={<span className="text-amber-400">{stars}</span>}
                      onSelect={() => selectDigit(i + 1)}
                    />
                  ))}
                  <MenuRow n={0} label="Back" onSelect={() => selectDigit(0)} />
                </>
              ) : menu === "label" ? (
                <>
                  {ctx.labelPresets.slice(0, 9).map((p, i) => (
                    <MenuRow key={p} n={i + 1} label={p} onSelect={() => selectDigit(i + 1)} />
                  ))}
                  <MenuRow n={0} label="Custom…" onSelect={() => selectDigit(0)} />
                </>
              ) : menu === "describe" ? (
                <>
                  {ctx.descriptionPresets.slice(0, 9).map((p, i) => (
                    <MenuRow key={p} n={i + 1} label={p} onSelect={() => selectDigit(i + 1)} />
                  ))}
                  <MenuRow n={0} label="Custom…" onSelect={() => selectDigit(0)} />
                </>
              ) : menu === "game" ? (
                <>
                  <MenuRow
                    n={1}
                    label={ctx.game ? `Keep "${ctx.game}"` : "Keep (no game detected)"}
                    onSelect={() => selectDigit(1)}
                  />
                  <MenuRow n={2} label="Edit (type)" onSelect={() => selectDigit(2)} />
                  <MenuRow
                    n={3}
                    label="Edit & remember"
                    disabled={!ctx.exe}
                    onSelect={() => selectDigit(3)}
                  />
                  <MenuRow n={0} label="Back" onSelect={() => selectDigit(0)} />
                </>
              ) : menu === "history" ? (
                <>
                  <div className="px-3 py-6 text-center text-white/40 text-[13px]">
                    No clips yet today
                  </div>
                  <MenuRow n={0} label="Back" onSelect={() => selectDigit(0)} />
                </>
              ) : menu === "app" ? (
                <>
                  <MenuRow
                    n={1}
                    label={
                      watchPaused === null
                        ? "Loading watcher state…"
                        : watchPaused
                          ? "Resume watching"
                          : "Pause watching"
                    }
                    disabled={watchPaused === null}
                    onSelect={() => selectDigit(1)}
                  />
                  <MenuRow n={2} label="Open ClipShelf window" onSelect={() => selectDigit(2)} />
                  <MenuRow n={3} label="Hide to tray" onSelect={() => selectDigit(3)} />
                  <MenuRow n={4} label="Wipe current clip" onSelect={() => selectDigit(4)} />
                  <MenuRow
                    n={5}
                    label={countUp.running ? "Stop count-up" : "Start count-up"}
                    onSelect={() => selectDigit(5)}
                  />
                  <MenuRow n={0} label="Back" onSelect={() => selectDigit(0)} />
                </>
              ) : menu === "timer" ? (
                <>
                  <MenuRow
                    n={1}
                    label={countUp.running ? "Stop count-up" : "Start count-up"}
                    onSelect={() => selectDigit(1)}
                  />
                  <MenuRow n={2} label="Reset" onSelect={() => selectDigit(2)} />
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
                : "Esc closes · press the number"}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
