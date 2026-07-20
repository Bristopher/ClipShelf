import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Tip } from "@/components/ui/tip";
import {
  Activity,
  History,
  Pause,
  Play,
  RotateCw,
  Timer as TimerIcon,
  TriangleAlert,
  Undo2,
} from "lucide-react";
import {
  wipeLog,
  toggleCountUp,
  undoLastAction,
  setWatchPaused,
  getDiagnostics,
  restartWatcher,
  openFolder,
  revealInExplorer,
} from "@/lib/commands";
import { useCountUp } from "@/hooks/useCountUp";
import { useWatcherStatus } from "@/hooks/useWatcherStatus";
import { useObsStatus } from "@/hooks/useObsStatus";
import { errorMessage, toastError, toastInfo } from "@/lib/toast";
import type { Diagnostics } from "@/types";

interface BottomBarProps {
  mode: "rename" | "sort";
  autoWipe: boolean;
  /** Show the OBS WebSocket connection dot (only when the integration is on). */
  obsEnabled: boolean;
  /** History view is currently swapped into the main area. */
  historyOpen: boolean;
  onAutoWipeChange: (value: boolean) => void;
  onWipe: () => void;
  onToggleHistory: () => void;
}

function fmt(secs: number) {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
}

/** Compact colored dot + label for the OBS WebSocket connection. */
function ObsDot() {
  const obs = useObsStatus();
  const color =
    obs.status === "connected"
      ? "bg-green-400"
      : obs.status === "connecting" || obs.status === "reconnecting"
        ? "bg-amber-400"
        : "bg-red-400";
  const label =
    obs.status === "connected"
      ? "OBS WebSocket connected"
      : obs.status === "connecting"
        ? "OBS WebSocket connecting..."
        : obs.status === "reconnecting"
          ? `OBS WebSocket reconnecting (attempt ${obs.attempt})...`
          : "OBS WebSocket disconnected";
  return (
    <Tip text={label}>
      <span className="flex items-center gap-1 text-t-muted">
        <span className={`h-1.5 w-1.5 rounded-full ${color}`} />
        OBS
      </span>
    </Tip>
  );
}

export function BottomBar({
  mode,
  autoWipe,
  obsEnabled,
  historyOpen,
  onAutoWipeChange,
  onWipe,
  onToggleHistory,
}: BottomBarProps) {
  const countUp = useCountUp();
  const watcher = useWatcherStatus();
  const paused = watcher.status === "paused";
  // "stopped" while NOT paused = watcher is dead or no folder configured —
  // that deserves red, not a reassuring "Watching" label.
  const stopped = watcher.status === "stopped";

  const handleWipe = async () => {
    try {
      await wipeLog();
      onWipe();
    } catch (e) {
      toastError(`Wipe failed: ${errorMessage(e)}`);
    }
  };

  const watchLabel = paused ? "Paused" : stopped ? "Stopped" : "Watching";
  const watchClass = paused
    ? "text-amber-400 hover:text-amber-300"
    : stopped
      ? "text-red-400 hover:text-red-300"
      : "";
  const watchTitle = paused
    ? "Watching paused — new clips are ignored. Click to resume."
    : stopped
      ? `Watcher is not running${watcher.restartCount > 0 ? ` (restarted ${watcher.restartCount}×)` : ""} — click to start it.`
      : `Watching for new clips${watcher.restartCount > 0 ? ` (auto-restarted ${watcher.restartCount}×)` : ""}. Click to pause.`;

  return (
    <div className="border-t border-t-border px-3 py-1.5 flex items-center gap-3 text-xs">
      <span className="text-t-muted">
        Mode: {mode === "rename" ? "Rename Only" : "Folder Sort"}
      </span>

      <Separator orientation="vertical" className="h-4" />

      <Button variant="ghost" size="sm" className="h-7 text-xs" onClick={handleWipe}>
        Wipe
      </Button>
      <Tip text={historyOpen ? "Back to live log" : "Show clip history"}>
        <Button
          variant="ghost"
          size="sm"
          className={`h-7 text-xs gap-1 ${historyOpen ? "bg-hover text-t-text" : ""}`}
          aria-pressed={historyOpen}
          onClick={onToggleHistory}
        >
          <History className="h-3 w-3" />
          History
        </Button>
      </Tip>
      <Tip text="Undo last move/rename">
        <Button
          variant="ghost"
          size="sm"
          className="h-7 text-xs gap-1"
          onClick={() => undoLastAction().catch((e) => toastError(errorMessage(e)))}
        >
          <Undo2 className="h-3 w-3" />
          Undo
        </Button>
      </Tip>

      <Separator orientation="vertical" className="h-4" />

      <Tip text={watchTitle}>
        <Button
          variant="ghost"
          size="sm"
          className={`h-7 text-xs gap-1 ${watchClass}`}
          // Stopped + resume share a path: setWatchPaused(false) (re)starts
          // the watcher when a folder is configured.
          onClick={() =>
            setWatchPaused(!paused && !stopped).catch((e) => toastError(errorMessage(e)))
          }
        >
          {paused ? (
            <Play className="h-3 w-3" />
          ) : stopped ? (
            <TriangleAlert className="h-3 w-3" />
          ) : (
            <Pause className="h-3 w-3" />
          )}
          {watchLabel}
        </Button>
      </Tip>

      {obsEnabled && (
        <>
          <Separator orientation="vertical" className="h-4" />
          <ObsDot />
        </>
      )}

      <Separator orientation="vertical" className="h-4" />

      <div className="flex items-center gap-1.5">
        <Tip text="Toggle count-up stopwatch (start ↔ reset)">
          <Button
            variant="ghost"
            size="sm"
            className="h-7 text-xs gap-1"
            onClick={() => toggleCountUp().catch((e) => toastError(errorMessage(e)))}
          >
            <TimerIcon className="h-3 w-3" />
            {countUp.running ? "Reset" : "Start ↑"}
          </Button>
        </Tip>
        <Tip text="Count-up stopwatch">
          <span
            className={`font-mono text-[11px] tabular-nums px-1.5 py-0.5 rounded ${
              countUp.running ? "text-t-text bg-panel" : "text-t-muted"
            }`}
          >
            {fmt(countUp.elapsedSecs)}
          </span>
        </Tip>
      </div>

      <Separator orientation="vertical" className="h-4" />

      <div className="flex items-center gap-1.5">
        <Switch
          id="auto-wipe"
          checked={autoWipe}
          onCheckedChange={onAutoWipeChange}
          className="scale-75"
        />
        <Label htmlFor="auto-wipe" className="text-xs text-t-muted cursor-pointer">
          Auto-Wipe
        </Label>
      </div>

      <div className="flex-1" />

      <DiagnosticsButton />
    </div>
  );
}

/** Row in the diagnostics popover — value clickable when onClick is set. */
function DiagRow({
  label,
  value,
  title,
  onClick,
}: {
  label: string;
  value: string;
  title?: string;
  onClick?: () => void;
}) {
  // Split the "Action\npath" title convention into label + dim sub line.
  const [tipText, tipSub] = title ? (title.split("\n") as [string, string?]) : [null, undefined];
  const inner = onClick ? (
    <button
      onClick={onClick}
      className="text-[10px] text-t-text truncate hover:underline underline-offset-2 text-left min-w-0 w-full"
    >
      {value}
    </button>
  ) : (
    <span className="text-[10px] text-t-text truncate min-w-0">{value}</span>
  );
  return (
    <div className="flex gap-2 items-baseline">
      <span className="text-[10px] text-t-muted w-20 shrink-0">{label}</span>
      {tipText ? (
        <Tip text={tipText} sub={tipSub} align="left" wrapperClass="min-w-0 flex-1">
          {inner}
        </Tip>
      ) : (
        inner
      )}
    </div>
  );
}

const SHORTCUTS: [string, string][] = [
  ["Del", "Wipe log"],
  ["Ctrl+Z", "Undo last move/rename"],
  ["Ctrl+F", "Filter log"],
  ["Ctrl+,", "Open Settings"],
];

/**
 * Diagnostics popover: watcher/OBS state, config + folder paths, a Restart
 * Watcher button, and the in-app shortcut cheat sheet. Snapshot fetched on
 * open — no polling.
 */
function DiagnosticsButton() {
  const [open, setOpen] = useState(false);
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  const wrapRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    getDiagnostics()
      .then(setDiag)
      .catch((e) => toastError(errorMessage(e)));
    const onDown = (e: MouseEvent) => {
      if (wrapRef.current && !wrapRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const handleRestart = () => {
    restartWatcher()
      .then(() => {
        toastInfo("Watcher restart requested");
        // Refresh the snapshot after the restart settles.
        window.setTimeout(() => {
          getDiagnostics().then(setDiag).catch(() => {});
        }, 500);
      })
      .catch((e) => toastError(errorMessage(e)));
  };

  const watcherLabel = diag
    ? diag.watchPaused
      ? "paused"
      : diag.watcherStatus +
        (diag.watcherRestartCount > 0 ? ` (restarted ${diag.watcherRestartCount}×)` : "")
    : "";

  return (
    <div ref={wrapRef} className="relative">
      <Tip text="Diagnostics" align="right">
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          aria-label="Diagnostics"
          onClick={() => setOpen((v) => !v)}
        >
          <Activity className="h-3.5 w-3.5" />
        </Button>
      </Tip>
      {open && (
        <div className="absolute bottom-full right-0 mb-2 z-50 w-72 rounded-md border border-t-border bg-panel shadow-lg p-3 space-y-2 animate-in fade-in-0 zoom-in-95 duration-150">
          <div className="flex items-center justify-between">
            <p className="text-[11px] font-semibold text-t-text">Diagnostics</p>
            {diag && <span className="text-[10px] text-t-muted">v{diag.version}</span>}
          </div>
          {diag ? (
            <>
              <div className="space-y-1">
                <DiagRow label="Watcher" value={watcherLabel} />
                <DiagRow
                  label="OBS WS"
                  value={diag.obsEnabled ? diag.obsStatus : "disabled"}
                />
                <DiagRow
                  label="Clips folder"
                  value={diag.videosFolder || "(not set)"}
                  title={diag.videosFolder ? `Open\n${diag.videosFolder}` : undefined}
                  onClick={
                    diag.videosFolder
                      ? () =>
                          openFolder(diag.videosFolder).catch((e) =>
                            toastError(errorMessage(e)),
                          )
                      : undefined
                  }
                />
                <DiagRow
                  label="Config"
                  value={diag.configPath}
                  title={`Reveal in Explorer\n${diag.configPath}`}
                  onClick={() =>
                    revealInExplorer(diag.configPath).catch((e) =>
                      toastError(errorMessage(e)),
                    )
                  }
                />
              </div>
              <Button
                variant="outline"
                size="sm"
                className="h-6 text-[10px] gap-1 w-full"
                onClick={handleRestart}
              >
                <RotateCw className="h-3 w-3" />
                Restart watcher
              </Button>
            </>
          ) : (
            <p className="text-[10px] text-t-muted">Loading...</p>
          )}
          <div className="pt-1 border-t border-t-border space-y-0.5">
            <p className="text-[10px] font-semibold text-t-muted">Shortcuts</p>
            {SHORTCUTS.map(([key, what]) => (
              <div key={key} className="flex gap-2 text-[10px]">
                <span className="font-mono text-t-text w-12 shrink-0">{key}</span>
                <span className="text-t-muted">{what}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
