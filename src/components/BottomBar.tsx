import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Pause, Play, Timer as TimerIcon, TriangleAlert, Undo2 } from "lucide-react";
import {
  wipeLog,
  restoreLog,
  toggleCountUp,
  undoLastAction,
  setWatchPaused,
} from "@/lib/commands";
import { useCountUp } from "@/hooks/useCountUp";
import { useWatcherStatus } from "@/hooks/useWatcherStatus";
import { useObsStatus } from "@/hooks/useObsStatus";
import { errorMessage, toastError } from "@/lib/toast";
import type { LogEntry } from "@/types";

interface BottomBarProps {
  mode: "rename" | "sort";
  autoWipe: boolean;
  /** Show the OBS WebSocket connection dot (only when the integration is on). */
  obsEnabled: boolean;
  onAutoWipeChange: (value: boolean) => void;
  onWipe: () => void;
  onRestore: (entries: LogEntry[]) => void;
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
    <span className="flex items-center gap-1 text-t-muted" title={label}>
      <span className={`h-1.5 w-1.5 rounded-full ${color}`} />
      OBS
    </span>
  );
}

export function BottomBar({
  mode,
  autoWipe,
  obsEnabled,
  onAutoWipeChange,
  onWipe,
  onRestore,
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

  const handleRestore = async () => {
    try {
      const restored = await restoreLog();
      onRestore(restored);
    } catch (e) {
      toastError(`Restore failed: ${errorMessage(e)}`);
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
      <Button variant="ghost" size="sm" className="h-7 text-xs" onClick={handleRestore}>
        Restore
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className="h-7 text-xs gap-1"
        onClick={() => undoLastAction().catch((e) => toastError(errorMessage(e)))}
        title="Undo last move/rename"
      >
        <Undo2 className="h-3 w-3" />
        Undo
      </Button>

      <Separator orientation="vertical" className="h-4" />

      <Button
        variant="ghost"
        size="sm"
        className={`h-7 text-xs gap-1 ${watchClass}`}
        // Stopped + resume share a path: setWatchPaused(false) (re)starts
        // the watcher when a folder is configured.
        onClick={() =>
          setWatchPaused(!paused && !stopped).catch((e) => toastError(errorMessage(e)))
        }
        title={watchTitle}
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

      {obsEnabled && (
        <>
          <Separator orientation="vertical" className="h-4" />
          <ObsDot />
        </>
      )}

      <Separator orientation="vertical" className="h-4" />

      <div className="flex items-center gap-1.5">
        <Button
          variant="ghost"
          size="sm"
          className="h-7 text-xs gap-1"
          onClick={() => toggleCountUp().catch((e) => toastError(errorMessage(e)))}
          title="Toggle count-up stopwatch (start ↔ reset)"
        >
          <TimerIcon className="h-3 w-3" />
          {countUp.running ? "Reset" : "Start ↑"}
        </Button>
        <span
          className={`font-mono text-[11px] tabular-nums px-1.5 py-0.5 rounded ${
            countUp.running ? "text-t-text bg-panel" : "text-t-muted"
          }`}
          title="Count-up stopwatch"
        >
          {fmt(countUp.elapsedSecs)}
        </span>
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
    </div>
  );
}
